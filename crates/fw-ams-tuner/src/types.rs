//! Tuner data model, shared by the service, the app backend and (via ts-rs) the UI.
//!
//! Every tunable value is an `f32` in its display unit (W, °C, MHz, %, CO steps) so the UI can
//! treat all parameters alike. `None` always means "leave the hardware default alone".

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ParamId {
    /// Sustained power limit (STAPM), W
    StapmW,
    /// Short boost power limit (PPT fast), W
    FastW,
    /// Medium boost power limit (PPT slow), W
    SlowW,
    /// CPU temperature limit (Tctl), °C
    TctlC,
    /// Curve Optimizer, all cores, steps (negative = undervolt)
    CoAll,
    /// Windows power plan: maximum processor frequency, MHz (0 = unlimited)
    MaxFreqMhz,
    /// Windows power plan: processor boost mode (PERFBOOSTMODE index)
    Boost,
    /// Windows power plan: energy performance preference, % (0 = performance)
    Epp,
}

impl ParamId {
    pub const ALL: [ParamId; 8] = [
        ParamId::StapmW,
        ParamId::FastW,
        ParamId::SlowW,
        ParamId::TctlC,
        ParamId::CoAll,
        ParamId::MaxFreqMhz,
        ParamId::Boost,
        ParamId::Epp,
    ];

    /// Values written through the SMU (need PawnIO); the rest go through the Windows power plan
    pub fn is_smu(self) -> bool {
        matches!(
            self,
            ParamId::StapmW | ParamId::FastW | ParamId::SlowW | ParamId::TctlC | ParamId::CoAll
        )
    }
}

/// Valid range and comparison tolerance of one parameter on this machine
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ParamSpec {
    pub id: ParamId,
    pub min: f32,
    pub max: f32,
    pub step: f32,
    /// Values above this get a warning in the UI (e.g. above the factory boost limit)
    pub warn_above: Option<f32>,
    /// Values below this get a warning (e.g. aggressive undervolt)
    pub warn_below: Option<f32>,
    /// Hardware reports the active value back
    pub readable: bool,
    /// Writable on this machine (SMU access available, family supported)
    pub supported: bool,
}

impl ParamSpec {
    pub fn clamp(&self, v: f32) -> f32 {
        let v = if self.step > 0.0 { (v / self.step).round() * self.step } else { v };
        v.clamp(self.min, self.max)
    }

    /// Readback is "the same" within this margin (SMU reports floats, limits are set in mW)
    pub fn tolerance(&self) -> f32 {
        match self.id {
            ParamId::StapmW | ParamId::FastW | ParamId::SlowW => 0.6,
            ParamId::TctlC => 1.0,
            _ => 0.5,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ParamStatus {
    /// Not touched by fw-ams, the firmware default is active
    Default,
    /// Requested but not sent yet
    Pending,
    /// Sent and the hardware reports the same value
    Applied,
    /// Sent, but the hardware has no way to report it back
    Unverified,
    /// Sent, but the hardware keeps a different value (BIOS/firmware limit)
    Limited,
    /// The firmware replaced the value and automatic restore is off
    Overridden,
    /// Sending failed
    Rejected,
    /// Not available on this machine
    Unsupported,
}

/// Full transparency for one parameter: what is default, wanted, sent and really active
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ParamState {
    pub id: ParamId,
    /// Firmware value for the current power source (observed before fw-ams changed it)
    pub default: Option<f32>,
    /// Value of the active profile, `None` = default
    pub desired: Option<f32>,
    /// Last value written to the hardware
    pub sent: Option<f32>,
    /// Value the hardware reports right now
    pub actual: Option<f32>,
    pub status: ParamStatus,
    /// How often the firmware overwrote the value and fw-ams restored it
    pub restored: u32,
    pub error: Option<String>,
    /// Unix ms of the last write
    #[ts(type = "number | null")]
    pub changed_ms: Option<u64>,
}

impl ParamState {
    pub fn new(id: ParamId) -> Self {
        Self {
            id,
            default: None,
            desired: None,
            sent: None,
            actual: None,
            status: ParamStatus::Default,
            restored: 0,
            error: None,
            changed_ms: None,
        }
    }
}

/// One set of tuning values. `None` = firmware default.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct TuneProfile {
    pub stapm_w: Option<f32>,
    pub fast_w: Option<f32>,
    pub slow_w: Option<f32>,
    pub tctl_c: Option<f32>,
    pub co_all: Option<f32>,
    pub max_freq_mhz: Option<f32>,
    pub boost: Option<f32>,
    pub epp: Option<f32>,
}

impl TuneProfile {
    pub fn get(&self, id: ParamId) -> Option<f32> {
        match id {
            ParamId::StapmW => self.stapm_w,
            ParamId::FastW => self.fast_w,
            ParamId::SlowW => self.slow_w,
            ParamId::TctlC => self.tctl_c,
            ParamId::CoAll => self.co_all,
            ParamId::MaxFreqMhz => self.max_freq_mhz,
            ParamId::Boost => self.boost,
            ParamId::Epp => self.epp,
        }
    }

    pub fn set(&mut self, id: ParamId, v: Option<f32>) {
        let slot = match id {
            ParamId::StapmW => &mut self.stapm_w,
            ParamId::FastW => &mut self.fast_w,
            ParamId::SlowW => &mut self.slow_w,
            ParamId::TctlC => &mut self.tctl_c,
            ParamId::CoAll => &mut self.co_all,
            ParamId::MaxFreqMhz => &mut self.max_freq_mhz,
            ParamId::Boost => &mut self.boost,
            ParamId::Epp => &mut self.epp,
        };
        *slot = v;
    }

    pub fn is_empty(&self) -> bool {
        ParamId::ALL.iter().all(|id| self.get(*id).is_none())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ProfileSlot {
    /// On charger
    Ac,
    /// On battery
    Dc,
    /// Battery saver (low battery), overrides `Dc`
    Saver,
}

/// Persisted tuner configuration (service: %ProgramData%\fw-ams\tuner.json)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct TunerConfig {
    pub ac: TuneProfile,
    pub dc: TuneProfile,
    pub saver: TuneProfile,
    /// Apply the profiles when the service starts (after boot)
    pub apply_on_boot: bool,
    /// Also apply the Curve Optimizer after boot (off by default: an unstable undervolt
    /// would otherwise crash on every boot)
    pub co_on_boot: bool,
    /// Restore values the firmware overwrote (e.g. on charger plug/unplug)
    pub auto_reapply: bool,
}

impl Default for TunerConfig {
    fn default() -> Self {
        Self {
            ac: TuneProfile::default(),
            dc: TuneProfile::default(),
            saver: TuneProfile::default(),
            apply_on_boot: true,
            co_on_boot: false,
            auto_reapply: true,
        }
    }
}

impl TunerConfig {
    pub fn profile(&self, slot: ProfileSlot) -> &TuneProfile {
        match slot {
            ProfileSlot::Ac => &self.ac,
            ProfileSlot::Dc => &self.dc,
            ProfileSlot::Saver => &self.saver,
        }
    }
}

/// A limit together with the value the SMU currently tracks against it
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LimitValue {
    pub limit: Option<f32>,
    pub value: Option<f32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CoreTelemetry {
    pub mhz: Option<f32>,
    pub volt: Option<f32>,
    pub power_w: Option<f32>,
    pub temp_c: Option<f32>,
}

/// Values read from the SMU power management table
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SmuTelemetry {
    #[ts(type = "number")]
    pub timestamp_ms: u64,
    /// Sustained power (STAPM): limit and time-averaged APU power, W
    pub stapm: LimitValue,
    /// PPT fast: limit and current APU power, W
    pub fast: LimitValue,
    /// PPT slow: limit and medium-averaged APU power, W
    pub slow: LimitValue,
    /// Tctl: limit and current CPU temperature, °C
    pub tctl: LimitValue,
    /// Total APU (socket) power, W – only where the table offset is known
    pub socket_w: Option<f32>,
    /// Per-core values – only where the table offsets are known
    pub cores: Vec<CoreTelemetry>,
    pub gfx_mhz: Option<f32>,
    pub fclk_mhz: Option<f32>,
    pub memclk_mhz: Option<f32>,
}

/// Values from the OS (no driver needed)
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct OsCpuTelemetry {
    /// Total CPU utilisation, %
    pub usage_percent: Option<f32>,
    /// Effective clock per logical processor, MHz
    pub thread_mhz: Vec<f32>,
    /// Utilisation per logical processor, %
    pub thread_usage: Vec<f32>,
    /// Nominal (base) clock, MHz
    pub base_mhz: Option<f32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum CpuFamily {
    Phoenix,
    HawkPoint,
    StrixPoint,
    KrackanPoint,
    StrixHalo,
    Other,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CpuInfo {
    /// Brand string, e.g. "AMD Ryzen AI 7 350 w/ Radeon 860M"
    pub name: String,
    pub family: CpuFamily,
    pub cores: u32,
    pub threads: u32,
    /// e.g. "93.37.0"
    pub smu_version: Option<String>,
    /// e.g. 0x650005
    pub pm_table_version: Option<u32>,
    /// Layout of the PM table is fully known (per-core values, socket power)
    pub pm_table_full: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Availability {
    /// Service running, SMU access works
    Ready,
    /// Simulated hardware (`--mock`)
    Mock,
    /// Only Windows power plan values (no SMU access)
    PowerPlanOnly,
    /// Helper service is not installed
    NotInstalled,
    /// Helper service is installed but not reachable
    ServiceStopped,
    /// PawnIO driver is not installed
    PawnIoMissing,
    /// RyzenSMU module file not found
    ModuleMissing,
    /// CPU family not supported for SMU tuning
    Unsupported,
    /// Only available on Windows for now
    PlatformUnsupported,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum EventKind {
    Info,
    Applied,
    /// Firmware overwrote a value
    Overridden,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TunerEvent {
    #[ts(type = "number")]
    pub timestamp_ms: u64,
    pub kind: EventKind,
    pub param: Option<ParamId>,
    /// German message for the UI
    pub message: String,
}

/// Curve Optimizer crash protection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", tag = "kind")]
#[ts(export)]
pub enum CoTrial {
    Idle,
    /// A new CO value is being tested; it counts as stable after `seconds` more seconds
    Running { seconds: u32 },
    /// The system went down during a trial: CO was not re-applied on boot
    CrashedLastTime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum PowerSource {
    Ac,
    Battery,
}

/// Everything the UI needs from the tuner
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TunerState {
    pub availability: Availability,
    /// German explanation for anything but `Ready`
    pub message: Option<String>,
    pub service_version: Option<String>,
    pub cpu: Option<CpuInfo>,
    pub power_source: PowerSource,
    pub active_slot: ProfileSlot,
    pub config: TunerConfig,
    pub specs: Vec<ParamSpec>,
    pub params: Vec<ParamState>,
    pub telemetry: Option<SmuTelemetry>,
    /// Newest last
    pub events: Vec<TunerEvent>,
    pub co_trial: CoTrial,
}

impl TunerState {
    pub fn unavailable(availability: Availability, message: impl Into<String>) -> Self {
        Self {
            availability,
            message: Some(message.into()),
            service_version: None,
            cpu: None,
            power_source: PowerSource::Ac,
            active_slot: ProfileSlot::Ac,
            config: TunerConfig::default(),
            specs: vec![],
            params: vec![],
            telemetry: None,
            events: vec![],
            co_trial: CoTrial::Idle,
        }
    }
}

pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
