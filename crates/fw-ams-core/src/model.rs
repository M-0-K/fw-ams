//! Data model shared by the CLI, the Tauri backend and (via ts-rs) the UI.
//!
//! Every value that may be unavailable is an `Option` – the UI shows "n/v" instead of fake data.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Everything the app knows at one point in time.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Snapshot {
    /// Unix time in milliseconds
    #[ts(type = "number")]
    pub timestamp_ms: u64,
    pub system: SystemInfo,
    pub access: AccessStatus,
    pub battery: Option<Battery>,
    pub charger: Charger,
    pub ports: Vec<PdPort>,
    pub thermal: Option<Thermal>,
    pub slots: Vec<Slot>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum DataSource {
    /// Framework embedded controller via framework_lib
    Ec,
    /// Linux sysfs / hwmon
    Sysfs,
    /// Generic OS battery API
    Os,
    Mock,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Model {
    Laptop12,
    Laptop13Intel,
    Laptop13CoreUltra,
    Laptop13Amd7040,
    Laptop13AmdAi300,
    Laptop16Amd7040,
    Laptop16AmdAi300,
    Desktop,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SystemInfo {
    pub model: Model,
    /// SMBIOS product name, e.g. "Laptop 13 (AMD Ryzen AI 300 Series)"
    pub product: Option<String>,
    pub bios_version: Option<String>,
    pub ec_version: Option<String>,
    pub pd_versions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AccessStatus {
    pub is_framework: bool,
    pub ec: EcState,
    /// Human readable hint how to fix missing access (German)
    pub hint: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum EcState {
    Ok,
    Unavailable,
    Mock,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ChargeState {
    Charging,
    Discharging,
    /// On AC, not charging (charge limit reached or full)
    Idle,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Battery {
    pub soc_percent: f32,
    pub state: ChargeState,
    pub critical: bool,
    pub voltage_v: Option<f32>,
    /// Positive while charging, negative while discharging
    pub current_a: Option<f32>,
    /// Positive while charging, negative while discharging
    pub power_w: Option<f32>,
    pub remaining_mah: Option<u32>,
    pub full_mah: Option<u32>,
    pub design_mah: Option<u32>,
    /// Last full charge capacity / design capacity
    pub health_percent: Option<f32>,
    pub cycle_count: Option<u32>,
    /// Upper charge limit (EC stops charging here)
    pub charge_limit_percent: Option<u8>,
    /// Lower end of the sustainer range; current Framework firmware ignores it
    pub charge_limit_min_percent: Option<u8>,
    /// Time until empty (discharging) or until the charge limit (charging)
    pub minutes_remaining: Option<u32>,
    pub temperature_c: Option<f32>,
    pub manufacturer: Option<String>,
    pub model: Option<String>,
    pub source: DataSource,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Charger {
    pub ac_present: bool,
    /// PD port index the laptop is charging from
    pub port: Option<u8>,
    pub voltage_v: Option<f32>,
    pub current_a: Option<f32>,
    /// Negotiated contract (V * A)
    pub contract_w: Option<f32>,
    pub epr: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Side {
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Position {
    Back,
    Middle,
    Front,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Location {
    pub side: Side,
    pub position: Position,
    /// German label, e.g. "Links hinten"
    pub label: String,
}

impl Location {
    pub fn new(side: Side, position: Position) -> Self {
        let s = match side {
            Side::Left => "Links",
            Side::Right => "Rechts",
        };
        let p = match position {
            Position::Back => "hinten",
            Position::Middle => "Mitte",
            Position::Front => "vorne",
        };
        Self {
            side,
            position,
            label: format!("{s} {p}"),
        }
    }
}

/// Power role of the laptop on that port.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum PowerRole {
    None,
    /// Laptop supplies power to the partner
    Source,
    /// Laptop receives power (charging)
    Sink,
}

/// Data role of the laptop on that port.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum DataRole {
    /// DFP – laptop is USB host
    Host,
    /// UFP – laptop is USB device
    Device,
}

/// What the PD controller sees on the other end of the CC lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Partner {
    Nothing,
    /// Partner consumes power (card, device)
    Sink,
    /// Partner supplies power (charger, dock)
    Source,
    DebugAccessory,
    AudioAccessory,
    PoweredAccessory,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PdPort {
    pub index: u8,
    pub location: Location,
    pub partner: Partner,
    pub power_role: PowerRole,
    pub data_role: Option<DataRole>,
    pub pd_contract: bool,
    pub voltage_v: f32,
    pub current_a: f32,
    pub power_w: f32,
    /// Maximum the partner advertises (only known when sinking)
    pub max_power_w: Option<f32>,
    pub vconn: bool,
    pub epr_active: bool,
    pub epr_supported: bool,
    pub dp_alt_mode: bool,
    pub alt_mode_raw: u8,
    /// The EC charges from this port
    pub active_charger: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Thermal {
    pub sensors: Vec<TempSensor>,
    pub fans: Vec<Fan>,
    pub throttle: Option<Throttle>,
    pub source: DataSource,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TempSensor {
    /// Raw sensor name, e.g. "cpu_f75303@4d"
    pub name: String,
    /// Friendly German label
    pub label: String,
    pub celsius: Option<f32>,
    /// Set when the sensor reports an error state
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Fan {
    pub name: String,
    pub rpm: Option<u32>,
    pub stalled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Throttle {
    /// Soft throttle (AMD SPPT / Intel)
    pub soft: bool,
    /// Hard throttle (PROCHOT#)
    pub hard: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum UsbSpeed {
    Low,
    Full,
    High,
    Super,
    SuperPlus,
    Unknown,
}

impl UsbSpeed {
    pub fn describe(self) -> &'static str {
        match self {
            UsbSpeed::Low => "USB 1.x Low Speed (1,5 Mbit/s)",
            UsbSpeed::Full => "USB 1.x/2.0 Full Speed (12 Mbit/s)",
            UsbSpeed::High => "USB 2.0 High Speed (480 Mbit/s)",
            UsbSpeed::Super => "USB 3.2 Gen 1 (5 Gbit/s)",
            UsbSpeed::SuperPlus => "USB 3.2 Gen 2 (10 Gbit/s)",
            UsbSpeed::Unknown => "USB (Geschwindigkeit unbekannt)",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct UsbDevice {
    pub vid: u16,
    pub pid: u16,
    /// "vvvv:pppp"
    pub id: String,
    pub product: Option<String>,
    pub speed: UsbSpeed,
    /// bcdUSB, e.g. "2.01"
    pub usb_version: String,
    /// Root port as ACPI path tail, e.g. "XHC0.PRT2" – platform independent
    pub root_port: Option<String>,
    pub port_chain: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum CardKind {
    /// Nothing detected – empty slot or passive USB-C card without device
    EmptyOrPassive,
    UsbC,
    UsbA,
    Hdmi,
    DisplayPort,
    Audio,
    Sd,
    MicroSd,
    Storage,
    Ethernet,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Confidence {
    /// Identified by USB ID on a calibrated port
    Certain,
    /// Inferred from PD state or an uncalibrated port
    Likely,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Card {
    pub kind: CardKind,
    pub confidence: Confidence,
    pub name: String,
    /// What this card type does / which protocols it uses
    pub features: Vec<String>,
    pub usb_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SlotCapabilities {
    pub usb4: Option<bool>,
    /// Maximum USB data rate in Gbit/s
    pub max_gbps: Option<u32>,
    pub display: Option<bool>,
    pub charging: Option<bool>,
    /// true when taken from official Framework documentation and checked
    pub verified: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Severity {
    Info,
    Warning,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Hint {
    pub severity: Severity,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Slot {
    /// Number as printed in Framework's documentation (1-based)
    pub number: u8,
    pub location: Location,
    pub pd_port: Option<u8>,
    pub capabilities: SlotCapabilities,
    pub card: Card,
    /// USB devices enumerated through this slot (card itself or attached devices)
    pub devices: Vec<UsbDevice>,
    /// Currently active protocols / modes
    pub protocols: Vec<String>,
    pub hints: Vec<Hint>,
}
