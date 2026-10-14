//! Raw data sources. Each source is optional; the collector merges what is available.

pub mod ec;
#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(not(target_os = "linux"))]
pub mod os_battery;
pub mod usb;
