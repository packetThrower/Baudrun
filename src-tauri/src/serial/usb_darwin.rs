//! macOS implementation of [`super::detect_missing_drivers`]. Uses
//! `/usr/sbin/ioreg -p IOUSB -l -w 0` to enumerate the USB registry,
//! because `enumerator.GetDetailedPortsList()` only surfaces ports
//! that already have a serial driver bound — the whole point of
//! missing-driver detection is to find devices the driver HASN'T
//! bound to yet.

use std::collections::HashSet;
use std::process::Command;
use std::sync::OnceLock;

use regex::Regex;

use super::chipsets::{self, USBSerialCandidate};
use super::detect::detect_suspect_enumerated_ports;

pub fn detect_missing_drivers() -> Result<Vec<USBSerialCandidate>, String> {
    // Build the "already drivered" set from the serialport enumerator
    // — every USB port that already has a /dev/cu.* node.
    let mut drivered: HashSet<String> = HashSet::new();
    if let Ok(ports) = serialport::available_ports() {
        for entry in ports {
            if let serialport::SerialPortType::UsbPort(u) = entry.port_type {
                drivered.insert(format!(
                    "{:04x}:{:04x}",
                    u.vid, u.pid
                ));
            }
        }
    }

    // libusb-direct stub returns empty today; once the CP210x-over-
    // rusb backend lands this will also pre-mark those VID:PID as
    // "handled without a driver."
    let usb_handled: HashSet<String> = HashSet::new();

    let devices = read_ioreg_usb()?;
    let mut missing = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    for d in devices {
        let info = chipsets::identify(&d.vid, &d.pid, &d.manufacturer);
        if !info.needs_driver() {
            continue;
        }
        let key = format!("{}:{}:{}", d.vid, d.pid, d.serial);
        if !seen.insert(key) {
            continue;
        }
        let vidpid = format!("{}:{}", d.vid, d.pid);
        if drivered.contains(&vidpid) || usb_handled.contains(&vidpid) {
            continue;
        }
        missing.push(USBSerialCandidate {
            vid: d.vid,
            pid: d.pid,
            chipset: info.name,
            manufacturer: d.manufacturer,
            product: d.product,
            serial_number: d.serial,
            driver_url: info.driver_url,
            reason: String::new(),
        });
    }

    // Also flag drivered-but-bad ports (counterfeit-detecting
    // Prolific drivers etc.).
    for c in detect_suspect_enumerated_ports() {
        let key = format!("{}:{}:{}", c.vid, c.pid, c.serial_number);
        if seen.contains(&key) {
            continue;
        }
        seen.insert(key);
        missing.push(c);
    }
    Ok(missing)
}

struct IoregDevice {
    vid: String,
    pid: String,
    manufacturer: String,
    product: String,
    serial: String,
}

fn read_ioreg_usb() -> Result<Vec<IoregDevice>, String> {
    let out = Command::new("/usr/sbin/ioreg")
        .args(["-p", "IOUSB", "-l", "-w", "0"])
        .output()
        .map_err(|e| format!("ioreg: {}", e))?;
    if !out.status.success() {
        return Err(format!(
            "ioreg exited {:?}",
            out.status.code()
        ));
    }
    Ok(parse_ioreg_usb(&String::from_utf8_lossy(&out.stdout)))
}

fn device_line_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    // Matches header lines like:
    //   | | +-o RUGGEDCOM USB Serial console@00123000  <class IOUSBHostDevice, ...>
    RE.get_or_init(|| Regex::new(r"\+-o\s+(.+?)(?:@[0-9a-fA-F]+)?\s+<class\s+(\w+)").unwrap())
}

fn prop_line_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    // Matches property lines like:
    //     "idVendor" = 2312
    //     "USB Product Name" = "RUGGEDCOM USB Serial console"
    RE.get_or_init(|| Regex::new(r#""([^"]+)"\s*=\s*(.+?)\s*$"#).unwrap())
}

fn parse_ioreg_usb(text: &str) -> Vec<IoregDevice> {
    let mut devices = Vec::new();
    let mut cur: Option<IoregDevice> = None;
    let flush = |cur: &mut Option<IoregDevice>, devices: &mut Vec<IoregDevice>| {
        if let Some(dev) = cur.take() {
            if !dev.vid.is_empty() && !dev.pid.is_empty() {
                devices.push(dev);
            }
        }
    };

    for line in text.lines() {
        if let Some(caps) = device_line_re().captures(line) {
            flush(&mut cur, &mut devices);
            let class = caps.get(2).map(|m| m.as_str()).unwrap_or_default();
            if class == "IOUSBHostDevice" || class == "IOUSBDevice" {
                cur = Some(IoregDevice {
                    vid: String::new(),
                    pid: String::new(),
                    manufacturer: String::new(),
                    product: String::new(),
                    serial: String::new(),
                });
            }
            continue;
        }
        let dev = match cur.as_mut() {
            Some(d) => d,
            None => continue,
        };
        let caps = match prop_line_re().captures(line) {
            Some(c) => c,
            None => continue,
        };
        let key = caps.get(1).map(|m| m.as_str()).unwrap_or("");
        let val = caps
            .get(2)
            .map(|m| m.as_str().trim().trim_matches('"'))
            .unwrap_or("");
        match key {
            "idVendor" => dev.vid = numeric_to_hex(val),
            "idProduct" => dev.pid = numeric_to_hex(val),
            "USB Vendor Name" | "kUSBVendorString" if dev.manufacturer.is_empty() => {
                dev.manufacturer = val.to_string();
            }
            "USB Product Name" | "kUSBProductString" if dev.product.is_empty() => {
                dev.product = val.to_string();
            }
            "USB Serial Number" | "kUSBSerialNumberString" if dev.serial.is_empty() => {
                dev.serial = val.to_string();
            }
            _ => {}
        }
    }
    flush(&mut cur, &mut devices);
    devices
}

/// Accepts ioreg's numeric property format (decimal "2312" or hex
/// "0x908") and returns a lowercase 4-digit hex string.
fn numeric_to_hex(s: &str) -> String {
    let trimmed = s.trim();
    let n = if let Some(hex) = trimmed.strip_prefix("0x").or_else(|| trimmed.strip_prefix("0X")) {
        i64::from_str_radix(hex, 16).ok()
    } else {
        trimmed.parse::<i64>().ok()
    };
    match n {
        Some(v) => format!("{:04x}", v),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_minimal_ioreg_entry() {
        let sample = r#"
    | +-o RUGGEDCOM USB Serial console@00123000  <class IOUSBHostDevice, id 0x100001234>
    | |   {
    | |     "idVendor" = 2312
    | |     "idProduct" = 511
    | |     "USB Vendor Name" = "Siemens"
    | |     "USB Product Name" = "RUGGEDCOM USB Serial console"
    | |     "USB Serial Number" = "ABC1234"
    | |   }
"#;
        let devices = parse_ioreg_usb(sample);
        assert_eq!(devices.len(), 1);
        let d = &devices[0];
        assert_eq!(d.vid, "0908");
        assert_eq!(d.pid, "01ff");
        assert_eq!(d.manufacturer, "Siemens");
        assert_eq!(d.product, "RUGGEDCOM USB Serial console");
        assert_eq!(d.serial, "ABC1234");
    }

    #[test]
    fn numeric_to_hex_accepts_both_radixes() {
        assert_eq!(numeric_to_hex("2312"), "0908");
        assert_eq!(numeric_to_hex("0x908"), "0908");
        assert_eq!(numeric_to_hex("not-a-number"), "");
    }
}
