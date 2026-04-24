//! Common driver-detection helpers shared by every platform's
//! `DetectMissingDrivers` implementation. Walks the OS enumerator
//! output and flags any port whose product string matches a
//! driver-issue placeholder (counterfeit-detecting Prolific drivers,
//! etc.) — those ports ARE drivered from the OS's perspective but
//! the driver is a stub that refuses to do I/O, so we want to warn
//! the user regardless.

use super::chipsets::{self, USBSerialCandidate};

/// Walk the enumerator and return any USB port whose product string
/// looks like a driver-issue placeholder. These are technically
/// drivered but the driver is refusing service, so the user still
/// needs to act.
pub fn detect_suspect_enumerated_ports() -> Vec<USBSerialCandidate> {
    let ports = match serialport::available_ports() {
        Ok(p) => p,
        Err(_) => return Vec::new(),
    };
    let mut out = Vec::new();
    for entry in ports {
        let u = match entry.port_type {
            serialport::SerialPortType::UsbPort(u) => u,
            _ => continue,
        };
        let product = u.product.clone().unwrap_or_default();
        if !chipsets::is_suspect_product(&product) {
            continue;
        }
        let vid = format!("{:04x}", u.vid);
        let pid = format!("{:04x}", u.pid);
        let info = chipsets::identify(&vid, &pid, "");
        if !info.needs_driver() {
            continue;
        }
        let mut candidate = USBSerialCandidate {
            vid: vid.clone(),
            pid: pid.clone(),
            chipset: info.name.clone(),
            manufacturer: String::new(),
            product,
            serial_number: u.serial_number.unwrap_or_default(),
            driver_url: info.driver_url.clone(),
            reason: String::new(),
        };
        // Prolific's current driver rejects pre-2016 chip revisions
        // with a scolding product string even when the chip is
        // genuine. Common with reputable older cables like
        // TRENDnet TU-S9.
        if vid == "067b" {
            candidate.chipset = "Prolific PL2303 (older chip revision)".into();
            candidate.reason =
                "Chip is likely genuine but Prolific's current driver refuses older revisions. Install your cable vendor's driver (e.g. TRENDnet) or Prolific's legacy driver.".into();
            candidate.driver_url =
                "https://www.prolific.com.tw/US/ShowProduct.aspx?p_id=225&pcid=41".into();
        }
        out.push(candidate);
    }
    out
}
