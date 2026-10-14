//! USB enumeration via nusb, with a platform independent root port identifier.
//!
//! The root port is expressed as the tail of its ACPI path ("XHC0.PRT2"):
//! - Windows: from the `ACPI(...)#ACPI(...)` location path
//! - Linux: from `firmware_node/path` of the root hub child in sysfs
//!
//! Both describe the same firmware object, so one calibration table serves both OSes.

use crate::model::{UsbDevice, UsbSpeed};
use nusb::MaybeFuture;

pub fn list() -> Vec<UsbDevice> {
    let devices = match nusb::list_devices().wait() {
        Ok(d) => d,
        Err(e) => {
            log::warn!("USB enumeration failed: {e}");
            return vec![];
        }
    };
    devices
        .map(|d| UsbDevice {
            vid: d.vendor_id(),
            pid: d.product_id(),
            id: format!("{:04x}:{:04x}", d.vendor_id(), d.product_id()),
            product: d.product_string().map(str::to_owned),
            speed: map_speed(d.speed()),
            usb_version: format_bcd(d.usb_version()),
            root_port: root_port(&d),
            port_chain: d.port_chain().to_vec(),
        })
        .collect()
}

fn map_speed(s: Option<nusb::Speed>) -> UsbSpeed {
    match s {
        Some(nusb::Speed::Low) => UsbSpeed::Low,
        Some(nusb::Speed::Full) => UsbSpeed::Full,
        Some(nusb::Speed::High) => UsbSpeed::High,
        Some(nusb::Speed::Super) => UsbSpeed::Super,
        Some(nusb::Speed::SuperPlus) => UsbSpeed::SuperPlus,
        _ => UsbSpeed::Unknown,
    }
}

fn format_bcd(v: u16) -> String {
    format!("{:x}.{:02x}", v >> 8, v & 0xff)
}

#[cfg(windows)]
fn root_port(d: &nusb::DeviceInfo) -> Option<String> {
    d.location_paths()
        .iter()
        .find_map(|p| acpi_root_port_windows(&p.to_string_lossy()))
}

#[cfg(target_os = "linux")]
fn root_port(d: &nusb::DeviceInfo) -> Option<String> {
    // sysfs path e.g. /sys/bus/usb/devices/3-2.1 -> root hub child is "3-2"
    let path = d.sysfs_path();
    let name = path.file_name()?.to_str()?;
    let (bus, chain) = name.split_once('-')?;
    let first = chain.split('.').next()?;
    let root_child = path.with_file_name(format!("{bus}-{first}"));
    let acpi = std::fs::read_to_string(root_child.join("firmware_node/path")).ok()?;
    acpi_root_port_dotted(acpi.trim())
}

#[cfg(not(any(windows, target_os = "linux")))]
fn root_port(_d: &nusb::DeviceInfo) -> Option<String> {
    None
}

/// "ACPI(_SB_)#ACPI(PCI0)#ACPI(GPPC)#ACPI(XHC0)#ACPI(RHUB)#ACPI(PRT2)" -> "XHC0.PRT2"
pub fn acpi_root_port_windows(location_path: &str) -> Option<String> {
    let segments: Vec<&str> = location_path
        .split('#')
        .filter_map(|s| s.strip_prefix("ACPI(")?.strip_suffix(')'))
        .collect();
    controller_and_port(&segments)
}

/// "\_SB_.PCI0.GPPC.XHC0.RHUB.PRT2" -> "XHC0.PRT2"
pub fn acpi_root_port_dotted(path: &str) -> Option<String> {
    let segments: Vec<&str> = path.trim_start_matches('\\').split('.').collect();
    controller_and_port(&segments)
}

fn controller_and_port(segments: &[&str]) -> Option<String> {
    let rhub = segments.iter().position(|s| s.eq_ignore_ascii_case("RHUB"))?;
    let controller = segments.get(rhub.checked_sub(1)?)?;
    let port = segments.get(rhub + 1)?;
    Some(format!("{controller}.{port}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_windows_location_path() {
        assert_eq!(
            acpi_root_port_windows(
                "ACPI(_SB_)#ACPI(PCI0)#ACPI(GPPC)#ACPI(XHC0)#ACPI(RHUB)#ACPI(PRT2)"
            )
            .as_deref(),
            Some("XHC0.PRT2")
        );
        assert_eq!(
            acpi_root_port_windows("PCIROOT(0)#PCI(0803)#PCI(0000)#USBROOT(0)#USB(2)"),
            None
        );
    }

    #[test]
    fn parses_linux_firmware_node_path() {
        assert_eq!(
            acpi_root_port_dotted("\\_SB_.PCI0.GP17.XHC1.RHUB.PRT3").as_deref(),
            Some("XHC1.PRT3")
        );
        assert_eq!(acpi_root_port_dotted("\\_SB_.PCI0"), None);
    }

    #[test]
    fn formats_bcd() {
        assert_eq!(format_bcd(0x0201), "2.01");
        assert_eq!(format_bcd(0x0320), "3.20");
    }
}
