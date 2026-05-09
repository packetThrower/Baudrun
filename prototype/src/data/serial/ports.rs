//! Serial port enumeration. Wraps `serialport::available_ports()`
//! with Baudrun-specific tweaks:
//!   - macOS `/dev/tty.*` nodes are filtered out (they're the
//!     blocking twin of `/dev/cu.*`; terminal apps should always use
//!     the callout device).
//!   - USB metadata (VID/PID/serial/product) is attached when the
//!     enumerator has it, plus a chipset annotation via
//!     [`crate::serial::chipsets::chipset_for_vid`].
//!   - libusb-direct entries are appended by the direct module
//!     (currently a stub — see `src-tauri/src/serial/direct.rs`).

use serde::{Deserialize, Serialize};

use super::chipsets;
use super::direct;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortInfo {
    pub name: String,
    #[serde(rename = "isUSB")]
    pub is_usb: bool,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub vid: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub pid: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub serial_number: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub product: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub chipset: String,
}

/// All available serial ports, sorted by name. Never returns `tty.*`
/// nodes on macOS. Enumeration failures bubble up; downstream
/// (ListPorts Tauri command) converts to a string for the frontend.
pub fn list_ports() -> Result<Vec<PortInfo>, serialport::Error> {
    let raw = serialport::available_ports()?;

    let mut out: Vec<PortInfo> = Vec::with_capacity(raw.len());
    let mut drivered_keys: Vec<String> = Vec::new();

    for entry in raw {
        if entry.port_name.starts_with("/dev/tty.") {
            continue;
        }
        let mut info = PortInfo {
            name: entry.port_name,
            is_usb: false,
            vid: String::new(),
            pid: String::new(),
            serial_number: String::new(),
            product: String::new(),
            chipset: String::new(),
        };
        if let serialport::SerialPortType::UsbPort(u) = entry.port_type {
            info.is_usb = true;
            info.vid = format!("{:04x}", u.vid);
            info.pid = format!("{:04x}", u.pid);
            info.serial_number = u.serial_number.unwrap_or_default();
            info.product = u.product.unwrap_or_default();
            info.chipset = chipsets::chipset_for_vid(&info.vid).to_string();
            drivered_keys.push(device_key(&info.vid, &info.pid, &info.serial_number));
        }
        out.push(info);
    }

    // Merge direct-USB entries (no-op stub today) — future rusb-backed
    // CP210x-without-driver path. Dedupe against the drivered set so
    // the same physical adapter doesn't appear twice when both paths
    // would work.
    for direct_info in direct::list_direct_usb() {
        if drivered_keys
            .iter()
            .any(|k| k == &device_key(&direct_info.vid, &direct_info.pid, &direct_info.serial_number))
        {
            continue;
        }
        out.push(direct_info);
    }

    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

fn device_key(vid: &str, pid: &str, serial: &str) -> String {
    format!(
        "{}:{}:{}",
        vid.to_ascii_lowercase(),
        pid.to_ascii_lowercase(),
        serial
    )
}
