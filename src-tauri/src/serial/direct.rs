//! libusb-direct serial backend — enumeration and port-name codec
//! for devices the OS's vendor driver isn't (or can't be) driving.
//! The actual chipset implementations live under
//! [`crate::usbserial`]; this module adapts them to the
//! enumerator / port-name conventions the rest of the serial layer
//! uses.
//!
//! A direct-USB port name has the form:
//!
//!   - `usb:VID:PID`            — device has no iSerial descriptor
//!   - `usb:VID:PID:Serial`     — preferred; unique across duplicates
//!
//! VID and PID are lowercase 4-hex-digit strings; Serial is the
//! iSerial descriptor value with NUL / whitespace padding stripped.

use std::io;
use std::sync::Arc;

use super::chipsets;
use super::ports::PortInfo;
use crate::usbserial::{self, Port as UsbPort};

pub const DIRECT_USB_PREFIX: &str = "usb:";

pub fn is_direct_usb_port_name(name: &str) -> bool {
    name.starts_with(DIRECT_USB_PREFIX)
}

/// Format a stable identifier for a direct-USB device. Using
/// VID+PID+Serial (rather than a bus/address tuple) keeps the name
/// stable across re-plugs whenever the device has an iSerial —
/// which all CP210x parts do by default.
pub fn format_direct_usb_port_name(device: &usbserial::Device) -> String {
    if device.serial.is_empty() {
        format!(
            "{}{:04x}:{:04x}",
            DIRECT_USB_PREFIX, device.vendor_id, device.product_id
        )
    } else {
        format!(
            "{}{:04x}:{:04x}:{}",
            DIRECT_USB_PREFIX, device.vendor_id, device.product_id, device.serial
        )
    }
}

/// Split `usb:VID:PID[:Serial]` into its fields. Returns an error
/// if the prefix is missing or the hex fields don't parse.
pub fn parse_direct_usb_port_name(name: &str) -> io::Result<(u16, u16, String)> {
    let tail = name
        .strip_prefix(DIRECT_USB_PREFIX)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, format!("not a direct-USB port name: {:?}", name)))?;
    let mut parts = tail.splitn(3, ':');
    let vid_s = parts.next().ok_or_else(|| malformed(name))?;
    let pid_s = parts.next().ok_or_else(|| malformed(name))?;
    let vid = u16::from_str_radix(vid_s, 16)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, format!("vid in {:?}: {}", name, e)))?;
    let pid = u16::from_str_radix(pid_s, 16)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, format!("pid in {:?}: {}", name, e)))?;
    let serial = parts.next().unwrap_or("").to_string();
    Ok((vid, pid, serial))
}

fn malformed(name: &str) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        format!("malformed direct-USB port name {:?}: want usb:VID:PID[:Serial]", name),
    )
}

/// Enumerate libusb-direct candidates and surface them as [`PortInfo`]
/// entries. Windows returns empty (the OS vendor driver is
/// authoritative there; see [`usbserial::list`]).
pub fn list_direct_usb() -> Vec<PortInfo> {
    let devices = match usbserial::list() {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    devices
        .into_iter()
        .map(|d| PortInfo {
            name: format_direct_usb_port_name(&d),
            is_usb: true,
            vid: format!("{:04x}", d.vendor_id),
            pid: format!("{:04x}", d.product_id),
            serial_number: d.serial.clone(),
            product: d.product.clone(),
            chipset: chipsets::chipset_for_vid(&format!("{:04x}", d.vendor_id)).to_string(),
        })
        .collect()
}

/// Find the [`usbserial::Device`] matching a direct-USB port name
/// and open it. Returns a shared handle so both the session's read
/// pump and its write path can access the same claimed interface
/// (libusb allows only one active claim per device).
pub fn open_direct_usb(port_name: &str) -> io::Result<Arc<dyn UsbPort>> {
    let (vid, pid, serial) = parse_direct_usb_port_name(port_name)?;
    let devices = usbserial::list().map_err(|e| io::Error::other(format!("enumerate USB: {}", e)))?;
    let device = devices
        .into_iter()
        .find(|d| d.vendor_id == vid && d.product_id == pid && (serial.is_empty() || d.serial == serial))
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!(
                    "USB device {:04x}:{:04x} (serial {:?}) not attached",
                    vid, pid, serial
                ),
            )
        })?;
    let port = usbserial::open(device)?;
    // Box<dyn Port> → Arc<dyn Port>. Needs Arc::from<Box<dyn Trait>>
    // which is stable for ?Sized unsized coercions.
    Ok(Arc::from(port))
}
