//! In-tree port of the Go `usbserial-go` library — a unified API for
//! talking to USB-to-serial adapters directly over libusb, bypassing
//! the OS vendor driver. Used on Linux and macOS as a fallback for
//! chipsets the kernel doesn't drive (notably CP210x on macOS
//! without SiLabs' VCP kext installed).
//!
//! Architecture mirrors the Go version:
//!
//!   - [`Port`] is the cross-chipset surface (Read + Write + line /
//!     framing / flow-control / DTR-RTS / break).
//!   - Each chipset implements [`Driver`] and registers itself at
//!     module-load time via [`register`].
//!   - [`list`] enumerates attached USB devices and returns one
//!     [`Device`] for each registered VID/PID match.
//!   - [`open`] dispatches to the chipset's driver to claim the
//!     device and hand back a boxed [`Port`].
//!
//! Windows builds are a passthrough — the OS vendor driver exposes
//! a COM port and `serialport` drives it natively, so libusb-direct
//! isn't used there. [`list`] returns an empty slice on Windows.

use std::io;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

pub mod cp210x;

pub type Vid = u16;
pub type Pid = u16;

/// Chipset family name. Matches the subpackage name in the Go
/// version (kept stable across the port so existing serialized
/// state round-trips).
pub type Chipset = &'static str;

pub const CHIPSET_CP210X: Chipset = "cp210x";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Parity {
    None,
    Odd,
    Even,
    Mark,
    Space,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlowControl {
    None,
    RtsCts,
    XonXoff,
}

/// Character-frame settings on the serial line. `stop_bits == 15`
/// encodes 1.5 stop bits (a CP210x quirk); otherwise 1 or 2.
#[derive(Debug, Clone, Copy)]
pub struct Framing {
    pub data_bits: u8,
    pub stop_bits: u8,
    pub parity: Parity,
}

impl Default for Framing {
    fn default() -> Self {
        Framing {
            data_bits: 8,
            stop_bits: 1,
            parity: Parity::None,
        }
    }
}

/// Snapshot of the input control lines at the moment of the query.
/// Not every chipset supports all four.
#[derive(Debug, Clone, Copy, Default)]
pub struct ModemStatus {
    pub cts: bool,
    pub dsr: bool,
    pub ri: bool,
    pub dcd: bool,
}

/// Cross-chipset serial-port surface. See module docs. Methods take
/// `&self` so a single port can be shared between a read thread and
/// the command handlers via `Arc`; implementations use interior
/// mutability (atomics + mutexes) for whatever state they need.
pub trait Port: Send + Sync {
    fn read(&self, buf: &mut [u8]) -> io::Result<usize>;
    fn write(&self, buf: &[u8]) -> io::Result<usize>;
    fn set_baud_rate(&self, baud: u32) -> io::Result<()>;
    fn set_framing(&self, f: Framing) -> io::Result<()>;
    fn set_flow_control(&self, fc: FlowControl) -> io::Result<()>;
    fn set_dtr(&self, assert: bool) -> io::Result<()>;
    fn set_rts(&self, assert: bool) -> io::Result<()>;
    fn modem_status(&self) -> io::Result<ModemStatus>;
    fn send_break(&self, duration: Duration) -> io::Result<()>;
    fn close(&self) -> io::Result<()>;
}

/// One attached USB-serial adapter, as returned by [`list`].
#[derive(Debug, Clone)]
pub struct Device {
    pub chipset: Chipset,
    pub vendor_id: Vid,
    pub product_id: Pid,
    pub serial: String,
    pub manufacturer: String,
    pub product: String,
    /// Platform-appropriate display identifier — on Linux/macOS a
    /// URI like `usb:bus=001:addr=004`; on Windows the OS COM name.
    pub path: String,
}

/// Chipset-specific entry point. Each driver registers once at
/// module init (see [`register`] callers in chipset submodules).
pub trait Driver: Send + Sync {
    fn name(&self) -> Chipset;
    fn matches(&self, vid: Vid, pid: Pid) -> bool;
    fn open(&self, device: Device) -> io::Result<Box<dyn Port>>;
}

fn driver_registry() -> &'static Mutex<Vec<&'static dyn Driver>> {
    static REG: OnceLock<Mutex<Vec<&'static dyn Driver>>> = OnceLock::new();
    REG.get_or_init(|| Mutex::new(Vec::new()))
}

/// Register a chipset driver. Intended to be called once at module
/// initialization (see [`cp210x::register`]).
pub fn register(driver: &'static dyn Driver) {
    driver_registry().lock().unwrap().push(driver);
}

/// Look up the first driver whose [`Driver::matches`] returns true
/// for the given VID/PID, or `None` if no registered driver claims it.
pub fn lookup_driver(vid: Vid, pid: Pid) -> Option<&'static dyn Driver> {
    driver_registry()
        .lock()
        .unwrap()
        .iter()
        .find(|d| d.matches(vid, pid))
        .copied()
}

/// Strip NUL padding and surrounding whitespace from a USB string
/// descriptor. Many vendors right-pad descriptors to a fixed width
/// with NULs or spaces (Siemens RUGGEDCOM pads Product to 40 chars).
pub fn trim_descriptor(s: &str) -> String {
    s.trim_matches(|c: char| c.is_whitespace() || c == '\0')
        .to_string()
}

/// Open the device via its chipset driver.
pub fn open(device: Device) -> io::Result<Box<dyn Port>> {
    match lookup_driver(device.vendor_id, device.product_id) {
        Some(driver) => driver.open(device),
        None => Err(io::Error::new(
            io::ErrorKind::Unsupported,
            format!(
                "no registered driver for {:04x}:{:04x}",
                device.vendor_id, device.product_id
            ),
        )),
    }
}

/// Enumerate attached USB-serial adapters a registered driver knows
/// how to handle.
#[cfg(any(target_os = "linux", target_os = "macos"))]
pub fn list() -> Result<Vec<Device>, rusb::Error> {
    cp210x::register();

    let devices = rusb::devices()?;
    let mut out = Vec::new();
    for device in devices.iter() {
        let desc = match device.device_descriptor() {
            Ok(d) => d,
            Err(_) => continue,
        };
        let vid = desc.vendor_id();
        let pid = desc.product_id();
        let Some(driver) = lookup_driver(vid, pid) else {
            continue;
        };

        let (manufacturer, product, serial) = read_string_descriptors(&device, &desc);

        out.push(Device {
            chipset: driver.name(),
            vendor_id: vid,
            product_id: pid,
            serial,
            manufacturer,
            product,
            path: format!(
                "usb:bus={:03}:addr={:03}",
                device.bus_number(),
                device.address()
            ),
        });
    }
    Ok(out)
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
pub fn list() -> Result<Vec<Device>, rusb::Error> {
    // On Windows the OS vendor driver serves the device as a COM
    // port, which `serialport` picks up natively — libusb-direct
    // isn't needed. Mirrors the Go version's platform split.
    Ok(Vec::new())
}

/// Open the device briefly to read iManufacturer / iProduct /
/// iSerial string descriptors. Returns empty strings for any
/// descriptor the device didn't provide or that can't be read.
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn read_string_descriptors(
    device: &rusb::Device<rusb::GlobalContext>,
    desc: &rusb::DeviceDescriptor,
) -> (String, String, String) {
    let Ok(handle) = device.open() else {
        return (String::new(), String::new(), String::new());
    };
    let timeout = Duration::from_millis(200);
    let lang = handle
        .read_languages(timeout)
        .ok()
        .and_then(|ls| ls.into_iter().next());

    let read = |idx: Option<u8>| -> String {
        let Some(idx) = idx else { return String::new() };
        if idx == 0 {
            return String::new();
        }
        let Some(lang) = lang else { return String::new() };
        handle
            .read_string_descriptor(lang, idx, timeout)
            .map(|s| trim_descriptor(&s))
            .unwrap_or_default()
    };

    (
        read(desc.manufacturer_string_index()),
        read(desc.product_string_index()),
        read(desc.serial_number_string_index()),
    )
}
