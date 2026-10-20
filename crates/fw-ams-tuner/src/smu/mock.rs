//! Simulated SMU for `--mock` and tests.
//!
//! Behaves like the real thing in the ways the UI cares about: limits read back, the
//! "firmware" resets STAPM every 90 s (like the EC on charger plug/unplug) and caps the fast
//! limit at 35 W (like a BIOS limit).

use super::{SmuBackend, SmuError};
use crate::family::{Mailbox, SmuMessages};
use crate::pm_table::READ_BYTES;
use crate::types::CpuFamily;
use std::time::Instant;

pub const MOCK_TABLE_VERSION: u32 = 0x005D_0009;
const FIRMWARE_RESET_EVERY_S: u64 = 90;
const BIOS_FAST_CAP: f32 = 35.0;

pub struct MockSmu {
    start: Instant,
    stapm: f32,
    fast: f32,
    slow: f32,
    tctl: f32,
    co_all: i32,
    last_reset: u64,
    pub cores: usize,
}

impl Default for MockSmu {
    fn default() -> Self {
        Self {
            start: Instant::now(),
            stapm: 28.0,
            fast: 35.0,
            slow: 33.0,
            tctl: 100.0,
            co_all: 0,
            last_reset: 0,
            cores: 8,
        }
    }
}

impl MockSmu {
    fn handle(&mut self, msg: u32, arg: u32) -> Result<(), SmuError> {
        let m = SmuMessages::for_family(CpuFamily::KrackanPoint).expect("known family");
        let w = arg as f32 / 1000.0;
        match msg {
            x if x == m.stapm => self.stapm = w,
            x if x == m.fast => self.fast = w.min(BIOS_FAST_CAP),
            x if x == m.slow => self.slow = w,
            x if x == m.tctl => self.tctl = arg as f32,
            x if x == m.co_all => {
                // Sign-extend the 20 bit value
                self.co_all = ((arg << 12) as i32) >> 12;
            }
            _ => return Err(SmuError::Response(0xFE)),
        }
        Ok(())
    }

    pub fn co_all(&self) -> i32 {
        self.co_all
    }
}

impl SmuBackend for MockSmu {
    fn code_name(&mut self) -> Result<u64, SmuError> {
        Ok(31)
    }

    fn smu_version(&mut self) -> Result<u32, SmuError> {
        Ok(0x005D_2500)
    }

    fn read_register(&mut self, _addr: u32) -> Result<u32, SmuError> {
        Err(SmuError::Io("mock has no registers".into()))
    }

    fn write_register(&mut self, _addr: u32, _value: u32) -> Result<(), SmuError> {
        Err(SmuError::Io("mock has no registers".into()))
    }

    fn send(&mut self, _mb: Mailbox, msg: u32, args: [u32; 6]) -> Result<[u32; 6], SmuError> {
        self.handle(msg, args[0])?;
        Ok(args)
    }

    fn pm_table(&mut self) -> Result<(u32, Vec<u8>), SmuError> {
        let t = self.start.elapsed().as_secs_f32();
        let secs = t as u64;
        if secs / FIRMWARE_RESET_EVERY_S > self.last_reset {
            self.last_reset = secs / FIRMWARE_RESET_EVERY_S;
            self.stapm = 28.0;
        }

        // Load pattern: bursts every 20 s
        let load = (0.5 + 0.5 * (t / 20.0 * std::f32::consts::TAU).sin()).powi(3);
        let jitter = (t * 7.3).sin() * 0.4;
        let fast_value = (4.0 + load * 40.0 + jitter).min(self.fast);
        let slow_value = (3.5 + load * 30.0).min(self.slow);
        let stapm_value = (3.0 + load * 22.0).min(self.stapm);
        let temp = (45.0 + load * 45.0 + jitter).min(self.tctl);

        let mut b = vec![0u8; READ_BYTES];
        let mut put = |off: usize, v: f32| b[off..off + 4].copy_from_slice(&v.to_le_bytes());
        put(0x00, self.stapm);
        put(0x04, stapm_value);
        put(0x08, self.fast);
        put(0x0C, fast_value);
        put(0x10, self.slow);
        put(0x14, slow_value);
        put(0x40, self.tctl);
        put(0x44, temp);
        put(0xD0, fast_value);
        put(0x4C0, 400.0 + load * 2300.0);
        put(0x4E0, 2000.0);
        put(0x4EC, 4000.0);
        let undervolt = self.co_all.min(0) as f32 * 0.004;
        for i in 0..self.cores.min(16) {
            let busy = if i < 4 { load } else { load * 0.6 };
            let phase = (t * 0.9 + i as f32).sin() * 0.1;
            put(0x9D8 + 4 * i, (0.3 + busy * 4.5 + phase).max(0.05));
            put(0xA08 + 4 * i, 0.75 + busy * 0.45 + undervolt);
            put(0xA38 + 4 * i, temp - 3.0 + phase * 10.0);
            put(0xA68 + 4 * i, 1.4 + busy * if i < 4 { 3.6 } else { 2.0 });
        }
        Ok((MOCK_TABLE_VERSION, b))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::family::{encode_arg, MP1_STRIX};
    use crate::types::ParamId;

    #[test]
    fn caps_fast_limit_and_decodes_co() {
        let mut m = MockSmu::default();
        m.send(MP1_STRIX, 0x15, [encode_arg(ParamId::FastW, 50.0), 0, 0, 0, 0, 0]).unwrap();
        assert_eq!(m.fast, BIOS_FAST_CAP);
        m.send(MP1_STRIX, 0x4C, [encode_arg(ParamId::CoAll, -12.0), 0, 0, 0, 0, 0]).unwrap();
        assert_eq!(m.co_all(), -12);
    }
}
