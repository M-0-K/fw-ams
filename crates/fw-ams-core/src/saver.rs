//! Automatic power saver: switches to a frugal profile while the battery is low.
//!
//! Pure state machine with hysteresis; the app performs the actual switch (Windows power mode,
//! tuner profile) and shows a notification for each [`SaverChange`].

use crate::model::{ChargeState, Snapshot};

/// Deactivate only once the charge is this far above the threshold again
const SOC_HYSTERESIS: f32 = 5.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaverChange {
    Activate,
    Deactivate,
}

#[derive(Debug, Default)]
pub struct SaverState {
    active: bool,
}

impl SaverState {
    pub fn active(&self) -> bool {
        self.active
    }

    /// `threshold` is the activation level in percent, `None` disables the saver
    /// (and deactivates it if it was on).
    pub fn evaluate(&mut self, s: &Snapshot, threshold: Option<u8>) -> Option<SaverChange> {
        let want = match (threshold, &s.battery) {
            (Some(t), Some(b)) => {
                let t = t as f32;
                let on_battery = b.state == ChargeState::Discharging && !s.charger.ac_present;
                if self.active {
                    on_battery && b.soc_percent <= t + SOC_HYSTERESIS
                } else {
                    on_battery && b.soc_percent <= t
                }
            }
            _ => false,
        };
        if want == self.active {
            return None;
        }
        self.active = want;
        Some(if want {
            SaverChange::Activate
        } else {
            SaverChange::Deactivate
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock;

    fn on_battery(soc: f32) -> Snapshot {
        let mut s = mock::snapshot();
        s.charger.ac_present = false;
        let b = s.battery.as_mut().unwrap();
        b.state = ChargeState::Discharging;
        b.soc_percent = soc;
        s
    }

    #[test]
    fn activates_below_threshold_with_hysteresis() {
        let mut st = SaverState::default();
        assert_eq!(st.evaluate(&on_battery(30.0), Some(20)), None);
        assert_eq!(st.evaluate(&on_battery(20.0), Some(20)), Some(SaverChange::Activate));
        assert_eq!(st.evaluate(&on_battery(19.0), Some(20)), None);
        // Within the hysteresis band it stays on
        assert_eq!(st.evaluate(&on_battery(24.0), Some(20)), None);
        assert_eq!(st.evaluate(&on_battery(26.0), Some(20)), Some(SaverChange::Deactivate));
    }

    #[test]
    fn deactivates_on_ac_and_when_disabled() {
        let mut st = SaverState::default();
        st.evaluate(&on_battery(10.0), Some(20));
        assert!(st.active());
        assert_eq!(st.evaluate(&mock::snapshot(), Some(20)), Some(SaverChange::Deactivate));

        st.evaluate(&on_battery(10.0), Some(20));
        assert_eq!(st.evaluate(&on_battery(10.0), None), Some(SaverChange::Deactivate));
    }
}
