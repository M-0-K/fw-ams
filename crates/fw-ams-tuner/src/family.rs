//! CPU family detection and SMU message tables.
//!
//! Message IDs, mailbox addresses and PM table offsets are taken from RyzenAdj
//! (https://github.com/FlyGoat/RyzenAdj, `lib/api.c` and `lib/nb_smu_ops.c`, LGPL-3.0),
//! as of v0.19. Only families listed in [`SmuMessages::for_family`] may be written to.

use crate::types::{CpuFamily, ParamId, ParamSpec};

/// SMU mailbox register addresses (accessed through the PawnIO RyzenSMU module)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mailbox {
    pub msg: u32,
    pub rsp: u32,
    pub args: u32,
}

/// MP1 mailbox set 3 (Strix Point, Krackan Point, Strix Halo)
pub const MP1_STRIX: Mailbox = Mailbox {
    msg: 0x03B1_0928,
    rsp: 0x03B1_0978,
    args: 0x03B1_0998,
};

/// MP1 message IDs for the writable parameters
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmuMessages {
    pub mailbox: Mailbox,
    pub stapm: u32,
    pub fast: u32,
    pub slow: u32,
    pub tctl: u32,
    pub co_all: u32,
}

impl SmuMessages {
    pub fn for_family(family: CpuFamily) -> Option<Self> {
        match family {
            // RyzenAdj groups these three in every setter we use
            CpuFamily::StrixPoint | CpuFamily::KrackanPoint | CpuFamily::StrixHalo => Some(Self {
                mailbox: MP1_STRIX,
                stapm: 0x14,
                fast: 0x15,
                slow: 0x16,
                tctl: 0x19,
                co_all: 0x4C,
            }),
            _ => None,
        }
    }

    pub fn message(&self, id: ParamId) -> Option<u32> {
        match id {
            ParamId::StapmW => Some(self.stapm),
            ParamId::FastW => Some(self.fast),
            ParamId::SlowW => Some(self.slow),
            ParamId::TctlC => Some(self.tctl),
            ParamId::CoAll => Some(self.co_all),
            _ => None,
        }
    }
}

/// Encode a parameter value as SMU message argument
pub fn encode_arg(id: ParamId, v: f32) -> u32 {
    match id {
        // Power limits are sent in mW
        ParamId::StapmW | ParamId::FastW | ParamId::SlowW => (v * 1000.0).round() as u32,
        ParamId::TctlC => v.round() as u32,
        // Curve Optimizer: 20 bit two's complement (RyzenAdj/G-Helper: 0x100000 - |v|)
        ParamId::CoAll => (v.round() as i32 as u32) & 0xF_FFFF,
        _ => v.round() as u32,
    }
}

/// Map the code name enum of the PawnIO RyzenSMU module
pub fn family_from_code_name(code: u64) -> CpuFamily {
    match code {
        23 => CpuFamily::Phoenix,
        30 => CpuFamily::HawkPoint,
        31 => CpuFamily::StrixPoint,
        32 => CpuFamily::StrixHalo,
        33 | 34 => CpuFamily::KrackanPoint,
        _ => CpuFamily::Other,
    }
}

/// Map CPUID family/model (used when the SMU is not reachable, e.g. for the UI hint)
pub fn family_from_cpuid(family: u32, model: u32) -> CpuFamily {
    match (family, model) {
        (0x19, 0x74 | 0x78) => CpuFamily::Phoenix,
        (0x19, 0x75 | 0x7C) => CpuFamily::HawkPoint,
        (0x1A, 0x20 | 0x24) => CpuFamily::StrixPoint,
        (0x1A, 0x60 | 0x68) => CpuFamily::KrackanPoint,
        (0x1A, 0x70) => CpuFamily::StrixHalo,
        _ => CpuFamily::Other,
    }
}

pub struct CpuId {
    pub name: String,
    pub family: u32,
    pub model: u32,
}

#[cfg(target_arch = "x86_64")]
pub fn cpuid() -> CpuId {
    use std::arch::x86_64::__cpuid;
    // SAFETY: CPUID is available on every x86_64 CPU
    let leaf1 = unsafe { __cpuid(1) };
    let base_family = (leaf1.eax >> 8) & 0xF;
    let base_model = (leaf1.eax >> 4) & 0xF;
    let (family, model) = if base_family == 0xF {
        (
            base_family + ((leaf1.eax >> 20) & 0xFF),
            (((leaf1.eax >> 16) & 0xF) << 4) | base_model,
        )
    } else {
        (base_family, base_model)
    };
    let mut bytes = Vec::with_capacity(48);
    for leaf in 0x8000_0002u32..=0x8000_0004 {
        // SAFETY: see above; AMD/Intel x86_64 CPUs implement the brand string leaves
        let r = unsafe { __cpuid(leaf) };
        for reg in [r.eax, r.ebx, r.ecx, r.edx] {
            bytes.extend_from_slice(&reg.to_le_bytes());
        }
    }
    let name = String::from_utf8_lossy(&bytes)
        .trim_matches(char::from(0))
        .trim()
        .to_string();
    CpuId { name, family, model }
}

#[cfg(not(target_arch = "x86_64"))]
pub fn cpuid() -> CpuId {
    CpuId {
        name: "unbekannt".into(),
        family: 0,
        model: 0,
    }
}

/// Safe ranges per family. Framework 13 AI 300 factory values: performance 30 W sustained /
/// 35 W boost (AMD cTDP 15–54 W). Values above the factory boost get a warning.
pub fn specs(family: CpuFamily, smu: bool) -> Vec<ParamSpec> {
    let smu_ok = smu && SmuMessages::for_family(family).is_some();
    let power = |id| ParamSpec {
        id,
        min: 5.0,
        max: 54.0,
        step: 0.5,
        warn_above: Some(35.0),
        warn_below: None,
        readable: true,
        supported: smu_ok,
    };
    let windows = cfg!(windows);
    vec![
        power(ParamId::StapmW),
        power(ParamId::FastW),
        power(ParamId::SlowW),
        ParamSpec {
            id: ParamId::TctlC,
            min: 60.0,
            max: 100.0,
            step: 1.0,
            warn_above: Some(95.0),
            warn_below: None,
            readable: true,
            supported: smu_ok,
        },
        ParamSpec {
            id: ParamId::CoAll,
            min: -30.0,
            max: 10.0,
            step: 1.0,
            warn_above: Some(0.0),
            warn_below: Some(-15.0),
            readable: false,
            supported: smu_ok,
        },
        ParamSpec {
            id: ParamId::MaxFreqMhz,
            min: 0.0,
            max: 6000.0,
            step: 100.0,
            warn_above: None,
            warn_below: None,
            readable: true,
            supported: windows,
        },
        ParamSpec {
            id: ParamId::Boost,
            min: 0.0,
            max: 6.0,
            step: 1.0,
            warn_above: None,
            warn_below: None,
            readable: true,
            supported: windows,
        },
        ParamSpec {
            id: ParamId::Epp,
            min: 0.0,
            max: 100.0,
            step: 1.0,
            warn_above: None,
            warn_below: None,
            readable: true,
            supported: windows,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_curve_optimizer_like_ryzenadj() {
        assert_eq!(encode_arg(ParamId::CoAll, -5.0), 0x100000 - 5);
        assert_eq!(encode_arg(ParamId::CoAll, 3.0), 3);
        assert_eq!(encode_arg(ParamId::StapmW, 15.5), 15_500);
    }

    #[test]
    fn krackan_shares_strix_messages() {
        let k = SmuMessages::for_family(CpuFamily::KrackanPoint).unwrap();
        assert_eq!(k, SmuMessages::for_family(CpuFamily::StrixPoint).unwrap());
        assert!(SmuMessages::for_family(CpuFamily::Phoenix).is_none());
    }
}
