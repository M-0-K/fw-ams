//! Parse the SMU power management (PM) table into [`SmuTelemetry`].
//!
//! The table is an array of little-endian `f32`. Offsets per table version come from RyzenAdj
//! (`lib/api.c`). Limit/value pairs at 0x00–0x14 are the same for all families; everything
//! else is only read for versions whose layout is known.

use crate::types::{CoreTelemetry, LimitValue, SmuTelemetry};

/// Bytes to request from the SMU (largest known table 0xD54, rounded up)
pub const READ_BYTES: usize = 0x1000;

struct Layout {
    tctl: Option<(usize, usize)>,
    socket_w: Option<usize>,
    /// Base offsets of the 16-entry per-core arrays: power, volt, temp, clock
    cores: Option<[usize; 4]>,
    gfx_mhz: Option<usize>,
    fclk: Option<usize>,
    memclk: Option<usize>,
}

fn layout(version: u32) -> Layout {
    match version {
        // Strix Point
        0x005D_0008 | 0x005D_0009 | 0x005D_000B => Layout {
            tctl: Some((0x40, 0x44)),
            socket_w: Some(0xD0),
            cores: Some([0x9D8, 0xA08, 0xA38, 0xA68]),
            gfx_mhz: Some(0x4C0),
            fclk: Some(0x4E0),
            memclk: Some(0x4EC),
        },
        // Krackan Point: RyzenAdj only knows the limits and Tctl so far
        0x0065_0005 => Layout {
            tctl: Some((0x40, 0x44)),
            socket_w: None,
            cores: None,
            gfx_mhz: None,
            fclk: None,
            memclk: None,
        },
        _ => Layout {
            tctl: None,
            socket_w: None,
            cores: None,
            gfx_mhz: None,
            fclk: None,
            memclk: None,
        },
    }
}

/// True when per-core values and socket power can be read for this table version
pub fn is_fully_known(version: u32) -> bool {
    layout(version).cores.is_some()
}

/// Read one float; NaN, infinities and absurd values become `None`
pub fn read_f32(table: &[u8], offset: usize) -> Option<f32> {
    let b = table.get(offset..offset + 4)?;
    let v = f32::from_le_bytes([b[0], b[1], b[2], b[3]]);
    (v.is_finite() && v.abs() < 1.0e6).then_some(v)
}

/// Convert the raw qwords returned by PawnIO into bytes
pub fn qwords_to_bytes(q: &[u64]) -> Vec<u8> {
    q.iter().flat_map(|w| w.to_le_bytes()).collect()
}

pub fn parse(version: u32, table: &[u8], cores: usize, timestamp_ms: u64) -> SmuTelemetry {
    let l = layout(version);
    let pair = |lim, val| LimitValue {
        limit: read_f32(table, lim),
        value: read_f32(table, val),
    };
    let opt = |o: Option<usize>| o.and_then(|o| read_f32(table, o));
    let core_list = match l.cores {
        Some([power, volt, temp, clk]) => (0..cores.min(16))
            .map(|i| CoreTelemetry {
                power_w: read_f32(table, power + 4 * i),
                volt: read_f32(table, volt + 4 * i),
                temp_c: read_f32(table, temp + 4 * i),
                // RyzenAdj prints GHz; accept MHz too in case a table version differs
                mhz: read_f32(table, clk + 4 * i).map(|v| if v < 100.0 { v * 1000.0 } else { v }),
            })
            .collect(),
        None => vec![],
    };
    SmuTelemetry {
        timestamp_ms,
        stapm: pair(0x00, 0x04),
        fast: pair(0x08, 0x0C),
        slow: pair(0x10, 0x14),
        tctl: l
            .tctl
            .map(|(a, b)| pair(a, b))
            .unwrap_or(LimitValue { limit: None, value: None }),
        socket_w: opt(l.socket_w),
        cores: core_list,
        gfx_mhz: opt(l.gfx_mhz),
        fclk_mhz: opt(l.fclk),
        memclk_mhz: opt(l.memclk),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table_with(values: &[(usize, f32)]) -> Vec<u8> {
        let mut t = vec![0u8; READ_BYTES];
        for (off, v) in values {
            t[*off..off + 4].copy_from_slice(&v.to_le_bytes());
        }
        t
    }

    #[test]
    fn krackan_reads_limits_and_tctl_only() {
        let t = table_with(&[(0x0, 28.0), (0x4, 12.5), (0x8, 35.0), (0xC, 18.0), (0x40, 100.0), (0x44, 61.0)]);
        let s = parse(0x0065_0005, &t, 8, 0);
        assert_eq!(s.stapm.limit, Some(28.0));
        assert_eq!(s.stapm.value, Some(12.5));
        assert_eq!(s.fast.value, Some(18.0));
        assert_eq!(s.tctl.value, Some(61.0));
        assert!(s.cores.is_empty());
        assert_eq!(s.socket_w, None);
        assert!(!is_fully_known(0x0065_0005));
    }

    #[test]
    fn strix_reads_cores() {
        let t = table_with(&[(0xD0, 20.0), (0x9D8 + 4, 3.5), (0xA68 + 4, 4.8)]);
        let s = parse(0x005D_0009, &t, 12, 0);
        assert_eq!(s.cores.len(), 12);
        assert_eq!(s.cores[1].power_w, Some(3.5));
        assert_eq!(s.cores[1].mhz, Some(4800.0));
        assert_eq!(s.socket_w, Some(20.0));
    }

    #[test]
    fn unknown_version_still_reads_generic_limits() {
        let t = table_with(&[(0x0, 15.0)]);
        let s = parse(0x1234, &t, 8, 0);
        assert_eq!(s.stapm.limit, Some(15.0));
        assert_eq!(s.tctl.limit, None);
    }

    #[test]
    fn rejects_garbage() {
        let t = table_with(&[(0x0, f32::NAN), (0x4, 1.0e9)]);
        let s = parse(0x0065_0005, &t, 8, 0);
        assert_eq!(s.stapm.limit, None);
        assert_eq!(s.stapm.value, None);
    }
}
