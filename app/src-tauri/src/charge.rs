//! Charge limit and charge current control through the EC (no admin rights needed).
//!
//! Keeps what was sent and compares it with what the EC reports, using the same status
//! model as the CPU tuner so the UI can show both the same way.
//!
//! Before the first write the EC's own limit is stored (`charge-restore.json`). When fw-ams
//! stops managing the limit, or on a reset, that value goes back to the EC.

use crate::settings::BatterySettings;
use crate::store;
use fw_ams_core::model::Snapshot;
use fw_ams_core::Collector;
use fw_ams_tuner::{now_ms, ParamStatus};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
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

    fn untrack(&mut self) {
        self.sent = None;
        self.status = ParamStatus::Default;
        self.error = None;
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
    /// Upper limit the EC had before fw-ams changed it (restored when management stops)
    pub original_max: Option<u8>,
}

/// What the EC had before fw-ams wrote to it
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct Original {
    /// (min %, max %)
    charge_limit: Option<(u8, u8)>,
}

pub struct ChargeController {
    mock: bool,
    store: Option<PathBuf>,
    original: Original,
    view: ChargeControl,
    sent_limit: Option<(u8, u8)>,
    /// `Some(None)` = limit removed
    sent_current: Option<Option<u32>>,
    last_ac: Option<bool>,
    last_ec: bool,
    /// Restoring the original failed; retried after reconnect or replug
    restore_failed: bool,
}

impl ChargeController {
    pub fn new(mock: bool, store: Option<PathBuf>) -> Self {
        let original: Original = store.as_deref().map(store::load).unwrap_or_default();
        Self {
            mock,
            store,
            view: ChargeControl {
                available: mock,
                max: ChargeParam::new(),
                min: ChargeParam::new(),
                current: ChargeParam::new(),
                design_mah: None,
                full_once_ms: None,
                original_max: original.charge_limit.map(|l| l.1),
            },
            original,
            sent_limit: None,
            sent_current: None,
            last_ac: None,
            last_ec: false,
            restore_failed: false,
        }
    }

    pub fn view(&self) -> ChargeControl {
        self.view.clone()
    }

    fn set_original(&mut self, limit: Option<(u8, u8)>) {
        self.original.charge_limit = limit;
        self.view.original_max = limit.map(|l| l.1);
        if let Some(path) = &self.store {
            let r = match limit {
                Some(_) => store::save(path, &self.original),
                None => store::remove(path),
            };
            if let Err(e) = r {
                log::warn!("charge restore file: {e}");
            }
        }
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
            self.restore_failed = false;
        }
        self.last_ac = Some(ac);
        self.last_ec = ec_ok;
        self.view.available = ec_ok;
        self.view.design_mah = s.battery.as_ref().and_then(|b| b.design_mah);
        let b = s.battery.as_ref();
        let ec_limit = if self.mock {
            Some(self.sent_limit.unwrap_or((0, 80)))
        } else {
            b.and_then(|b| b.charge_limit_percent.map(|max| (b.charge_limit_min_percent.unwrap_or(0), max)))
        };

        // Charge limit
        let want_limit = cfg.manage_charge_limit.then_some((cfg.charge_min, cfg.charge_max));
        self.view.max.desired = want_limit.map(|l| l.1 as f32);
        self.view.min.desired = want_limit.map(|l| l.0 as f32);
        if let Some((min, max)) = want_limit {
            if ec_ok && self.sent_limit != Some((min, max)) {
                // Remember the EC's own value once, before fw-ams changes it
                if self.original.charge_limit.is_none() {
                    if let Some(own) = ec_limit.filter(|l| *l != (min, max)) {
                        self.set_original(Some(own));
                    }
                }
                let r = match ec {
                    Some(ec) => ec.set_charge_limit(min, max),
                    None => Ok(()),
                };
                self.sent_limit = Some((min, max));
                for (p, v) in [(&mut self.view.max, max), (&mut self.view.min, min)] {
                    p.sent = Some(v as f32);
                    p.error = r.as_ref().err().cloned();
                    p.status = if r.is_ok() { ParamStatus::Pending } else { ParamStatus::Rejected };
                }
                match &r {
                    Ok(()) => log::info!("charge limit set to {min}..{max} % ({})", now_ms()),
                    Err(e) => log::warn!("charge limit: {e}"),
                }
            }
        } else {
            if self.sent_limit.take().is_some() {
                self.view.max.untrack();
                self.view.min.untrack();
            }
            // Management is off: give the EC its own value back
            if let Some((min, max)) = self.original.charge_limit.filter(|_| ec_ok && !self.restore_failed) {
                let r = match ec {
                    Some(ec) => ec.set_charge_limit(min, max),
                    None => Ok(()),
                };
                match r {
                    Ok(()) => {
                        log::info!("charge limit restored to {min}..{max} %");
                        self.set_original(None);
                    }
                    Err(e) => {
                        log::warn!("charge limit restore: {e}");
                        self.restore_failed = true;
                        self.view.max.error = Some(format!("Ursprüngliches Limit nicht wiederhergestellt: {e}"));
                    }
                }
            }
        }
        let actual = if self.mock { Some(self.sent_limit.unwrap_or((0, 80))) } else { ec_limit };
        Self::judge(&mut self.view.max, actual.map(|l| f32::from(l.1)));
        Self::judge(&mut self.view.min, actual.map(|l| f32::from(l.0)));

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
            self.sent_current = want_current.map(Some);
            let p = &mut self.view.current;
            p.sent = want_current.map(|c| c as f32);
            p.error = r.as_ref().err().cloned();
            p.status = match (&r, want_current) {
                (Err(_), _) => ParamStatus::Rejected,
                (Ok(()), Some(_)) => ParamStatus::Unverified,
                (Ok(()), None) => ParamStatus::Default,
            };
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

    /// Undo everything fw-ams wrote: the original charge limit goes back and the charge
    /// current limit is removed (the EC may still hold one from an earlier run). Call after
    /// the battery settings were reset, so the next `sync` does not write again.
    pub fn restore(&mut self, collector: Option<&Collector>) -> Result<String, String> {
        let ec = collector.and_then(|c| c.ec());
        if !self.mock && ec.is_none() {
            return Err("Kein EC-Zugriff – das Ladelimit gilt bis zur nächsten Änderung im BIOS.".into());
        }
        let mut done = vec![];
        match self.original.charge_limit {
            Some((min, max)) => {
                if let Some(ec) = ec {
                    ec.set_charge_limit(min, max)?;
                }
                self.set_original(None);
                done.push(format!("Ladelimit auf {max} % zurückgesetzt"));
            }
            None => done.push("Ladelimit war unverändert".into()),
        }
        if let Some(ec) = ec {
            ec.set_charge_current_limit(None)?;
        }
        done.push("Ladestrom ohne Limit".into());
        self.sent_limit = None;
        self.sent_current = None;
        for p in [&mut self.view.max, &mut self.view.min, &mut self.view.current] {
            p.desired = None;
            p.untrack();
        }
        Ok(done.join(", "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fw_ams_core::mock;

    fn managed(max: u8) -> BatterySettings {
        BatterySettings {
            manage_charge_limit: true,
            charge_max: max,
            charge_rate: 0.5,
            ..Default::default()
        }
    }

    #[test]
    fn mock_applies_and_reads_back() {
        let mut c = ChargeController::new(true, None);
        c.sync(None, &mock::snapshot(), &managed(70));
        let v = c.view();
        assert_eq!(v.max.sent, Some(70.0));
        assert_eq!(v.max.status, ParamStatus::Applied);
        assert_eq!(v.current.sent, Some(1958.0));
        assert_eq!(v.current.status, ParamStatus::Unverified);
        // The mock EC had 80 % before
        assert_eq!(v.original_max, Some(80));

        let off = BatterySettings::default();
        c.sync(None, &mock::snapshot(), &off);
        let v = c.view();
        assert_eq!(v.max.status, ParamStatus::Default);
        assert_eq!(v.current.sent, None);
        assert_eq!(v.original_max, None);
    }

    #[test]
    fn original_limit_is_persisted_and_restored() {
        let path = std::env::temp_dir().join(format!("fw-ams-charge-{}.json", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let mut c = ChargeController::new(true, Some(path.clone()));
        c.sync(None, &mock::snapshot(), &managed(60));
        drop(c);

        // App restart: the original is still known
        let mut c = ChargeController::new(true, Some(path.clone()));
        assert_eq!(c.view().original_max, Some(80));
        assert_eq!(c.restore(None).unwrap(), "Ladelimit auf 80 % zurückgesetzt, Ladestrom ohne Limit");
        assert!(!path.exists());
        assert_eq!(c.view().current.desired, None);
    }
}
