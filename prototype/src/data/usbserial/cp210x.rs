//! SiLabs CP210x USB-to-UART bridge driver — direct-libusb
//! implementation on Linux and macOS. Windows falls through to the
//! OS vendor driver via `serialport` and isn't exercised here.
//!
//! Reference: SiLabs AN571 "CP210x/CP211x USB to UART Bridge VCP
//! Interface Specification".
//! <https://www.silabs.com/documents/public/application-notes/AN571.pdf>

use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use rusb::{Direction, GlobalContext, TransferType};

use super::{
    register as register_driver, trim_descriptor, Device, Driver, FlowControl, Framing, ModemStatus,
    Parity, Pid, Port, Vid, CHIPSET_CP210X,
};

// --- USB identifiers -----------------------------------------------------

/// SiLabs vendor ID — every stock CP210x-family chip ships under this VID.
pub const VENDOR_ID: Vid = 0x10C4;

/// Product IDs for CP210x variants we know. Not exhaustive; add
/// entries as hardware shows up. Source: Linux kernel's
/// `drivers/usb/serial/cp210x.c` `id_table[]` + SiLabs AN571.
const PRODUCT_IDS: &[Pid] = &[
    0xEA60, // CP2102 / CP2102N / CP2104
    0xEA70, // CP2105 (dual UART)
    0xEA71, // CP2108 (quad UART)
    0xEA80, // CP2110 (HID-to-UART variant)
];

/// CP210x-based devices reflashed under a non-SiLabs USB-IF VID.
/// The wire protocol is still stock CP210x; only the descriptors
/// are different.
const REBRANDS: &[(Vid, Pid)] = &[
    // Siemens RUGGEDCOM USB Serial console (RST2228 and similar).
    // Confirmed CP210x by the device's "USB Vendor Name" = "Silicon
    // Labs" descriptor.
    (0x0908, 0x01FF),
];

// --- AN571 control-request codes ----------------------------------------

const REQ_IFC_ENABLE: u8 = 0x00;
const REQ_SET_LINE_CTL: u8 = 0x03;
const REQ_SET_BREAK: u8 = 0x05;
const REQ_SET_MHS: u8 = 0x07;
const REQ_GET_MDM_STS: u8 = 0x08;
const REQ_SET_FLOW: u8 = 0x13;
const REQ_SET_BAUD_RATE: u8 = 0x1E;

const IFC_DISABLE: u16 = 0x00;
const IFC_ENABLE: u16 = 0x01;

// SET_MHS wValue layout: low byte = DTR/RTS values; high byte =
// per-line mask bits indicating which bits the chip should apply.
const MHS_DTR: u16 = 0x0001;
const MHS_RTS: u16 = 0x0002;
const MHS_DTR_MASK: u16 = 0x0100;
const MHS_RTS_MASK: u16 = 0x0200;

// GET_MDM_STS result byte bits.
const MDM_CTS: u8 = 0x10;
const MDM_DSR: u8 = 0x20;
const MDM_RI: u8 = 0x40;
const MDM_DCD: u8 = 0x80;

// bmRequestType for vendor interface-recipient control transfers.
const CTRL_OUT: u8 = 0x41; // host → device
const CTRL_IN: u8 = 0xC1; // device → host

const CTRL_TIMEOUT: Duration = Duration::from_millis(500);
/// Short enough that Close observes `closed = true` promptly; long
/// enough not to burn CPU spinning. Matches the native backend.
const READ_TIMEOUT: Duration = Duration::from_millis(100);
const WRITE_TIMEOUT: Duration = Duration::from_secs(1);

// --- Driver registration ------------------------------------------------

struct Cp210xDriver;

impl Driver for Cp210xDriver {
    fn name(&self) -> super::Chipset {
        CHIPSET_CP210X
    }

    fn matches(&self, vid: Vid, pid: Pid) -> bool {
        (vid == VENDOR_ID && PRODUCT_IDS.contains(&pid))
            || REBRANDS.iter().any(|(v, p)| *v == vid && *p == pid)
    }

    fn open(&self, device: Device) -> io::Result<Box<dyn Port>> {
        open_port(device)
    }
}

/// Register the driver once (idempotent). Called by
/// [`super::list`] as a safety net in case setup forgot.
pub fn register() {
    static ONCE: OnceLock<()> = OnceLock::new();
    ONCE.get_or_init(|| {
        static DRIVER: Cp210xDriver = Cp210xDriver;
        register_driver(&DRIVER);
    });
}

// --- Port implementation -------------------------------------------------

struct Cp210xPort {
    /// libusb handle owning the interface claim. `rusb::DeviceHandle`
    /// is `Send + Sync` and exposes `&self` methods — no outer Mutex
    /// is needed, and having one would serialize bulk reads against
    /// bulk writes (which the underlying libusb happily runs
    /// concurrently on separate endpoints).
    handle: rusb::DeviceHandle<GlobalContext>,

    /// Serializes control transfers against each other (SetBaudRate +
    /// SetDTR from different command handlers overlapping). Bulk
    /// read/write don't take this lock.
    ctrl_mu: Mutex<()>,

    iface: u16,
    in_endpoint: u8,
    out_endpoint: u8,

    dtr: AtomicBool,
    rts: AtomicBool,
    closed: AtomicBool,
}

/// Locate, open, and configure the CP210x matching `target`. On
/// success, returns a ready port — UART enabled, 9600-8N1, no flow,
/// DTR+RTS asserted (matches `go.bug.st/serial` on open). Boxed via
/// `dyn Port` so callers don't need to name the private struct.
pub fn open_port(target: Device) -> io::Result<Box<dyn Port>> {
    Ok(Box::new(open_port_inner(target)?))
}

fn open_port_inner(target: Device) -> io::Result<Cp210xPort> {
    let device = find_device(&target)?;
    let handle = device.open().map_err(rusb_to_io)?;

    // Claim the kernel driver on Linux if one is bound (on macOS
    // this call is both unnecessary and sometimes fails with
    // LIBUSB_ERROR_ACCESS on composite devices — skip there).
    #[cfg(target_os = "linux")]
    {
        let _ = handle.set_auto_detach_kernel_driver(true);
    }

    // CP210x only ever has one configuration; assert it.
    let _ = handle.set_active_configuration(1);

    // Find the first interface's first bulk IN+OUT endpoint pair.
    let config = device.active_config_descriptor().map_err(rusb_to_io)?;
    let iface_desc = config
        .interfaces()
        .next()
        .and_then(|i| i.descriptors().next())
        .ok_or_else(|| io::Error::other("cp210x: no usable interface"))?;

    let iface = iface_desc.interface_number() as u16;
    let (in_ep, out_ep) = find_bulk_endpoints(&iface_desc)?;

    handle
        .claim_interface(iface as u8)
        .map_err(rusb_to_io)?;

    let port = Cp210xPort {
        handle,
        ctrl_mu: Mutex::new(()),
        iface,
        in_endpoint: in_ep,
        out_endpoint: out_ep,
        dtr: AtomicBool::new(true),
        rts: AtomicBool::new(true),
        closed: AtomicBool::new(false),
    };

    // Bring-up sequence. Any failure tears down the UART so we
    // don't leak an enabled port after a failed Open.
    if let Err(err) = port.bring_up() {
        port.teardown();
        return Err(err);
    }
    Ok(port)
}

impl Cp210xPort {
    fn bring_up(&self) -> io::Result<()> {
        self.set_ifc_enable(true)?;
        self.set_baud_rate_inner(9600)?;
        self.set_framing_inner(Framing::default())?;
        self.set_flow_control_inner(FlowControl::None)?;
        self.write_mhs(true, true, true, true)?;
        Ok(())
    }

    fn teardown(&self) {
        // Fire-and-forget; device may already be gone (physical
        // unplug) so errors here are expected.
        let _ = self.handle.release_interface(self.iface as u8);
    }

    fn set_ifc_enable(&self, enable: bool) -> io::Result<()> {
        let value = if enable { IFC_ENABLE } else { IFC_DISABLE };
        self.control_out(REQ_IFC_ENABLE, value, &[])
    }

    fn set_baud_rate_inner(&self, baud: u32) -> io::Result<()> {
        if baud == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "cp210x: baud rate must be positive",
            ));
        }
        let payload = baud.to_le_bytes();
        self.control_out(REQ_SET_BAUD_RATE, 0, &payload)
    }

    fn set_framing_inner(&self, f: Framing) -> io::Result<()> {
        if !(5..=8).contains(&f.data_bits) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("cp210x: data bits must be 5..8, got {}", f.data_bits),
            ));
        }
        let stop_code: u16 = match f.stop_bits {
            1 => 0,
            15 => 1,
            2 => 2,
            other => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("cp210x: stop bits must be 1, 15 (=1.5), or 2; got {}", other),
                ));
            }
        };
        let parity_code: u16 = match f.parity {
            Parity::None => 0,
            Parity::Odd => 1,
            Parity::Even => 2,
            Parity::Mark => 3,
            Parity::Space => 4,
        };
        let value: u16 = (u16::from(f.data_bits) << 8) | (parity_code << 4) | stop_code;
        self.control_out(REQ_SET_LINE_CTL, value, &[])
    }

    fn set_flow_control_inner(&self, fc: FlowControl) -> io::Result<()> {
        // AN571 §5.17: SET_FLOW takes a 16-byte payload of four u32
        // fields in LE order. Presets below match what the Linux
        // kernel's cp210x.c sends for the same three modes —
        // widely field-tested.
        let (ctrl, replace): (u32, u32) = match fc {
            FlowControl::None => (0x0000_0001, 0x0000_0040),
            FlowControl::RtsCts => (0x0000_0009, 0x0000_0080),
            FlowControl::XonXoff => (0x0000_0181, 0x0000_0040),
        };
        let mut payload = [0u8; 16];
        payload[0..4].copy_from_slice(&ctrl.to_le_bytes());
        payload[4..8].copy_from_slice(&replace.to_le_bytes());
        payload[8..12].copy_from_slice(&0x80u32.to_le_bytes()); // XonLimit
        payload[12..16].copy_from_slice(&0x80u32.to_le_bytes()); // XoffLimit
        self.control_out(REQ_SET_FLOW, 0, &payload)
    }

    fn write_mhs(&self, dtr: bool, rts: bool, dtr_mask: bool, rts_mask: bool) -> io::Result<()> {
        let mut value: u16 = 0;
        if dtr {
            value |= MHS_DTR;
        }
        if rts {
            value |= MHS_RTS;
        }
        if dtr_mask {
            value |= MHS_DTR_MASK;
        }
        if rts_mask {
            value |= MHS_RTS_MASK;
        }
        self.control_out(REQ_SET_MHS, value, &[])
    }

    fn control_out(&self, request: u8, value: u16, data: &[u8]) -> io::Result<()> {
        let _guard = self.ctrl_mu.lock().unwrap();
        let n = self
            .handle
            .write_control(CTRL_OUT, request, value, self.iface, data, CTRL_TIMEOUT)
            .map_err(|e| {
                io::Error::other(format!("cp210x: control OUT req={:02x}: {}", request, e))
            })?;
        if n != data.len() {
            return Err(io::Error::other(format!(
                "cp210x: control OUT req={:02x}: short transfer {}/{}",
                request,
                n,
                data.len()
            )));
        }
        Ok(())
    }

    fn control_in(&self, request: u8, value: u16, data: &mut [u8]) -> io::Result<()> {
        let _guard = self.ctrl_mu.lock().unwrap();
        let n = self
            .handle
            .read_control(CTRL_IN, request, value, self.iface, data, CTRL_TIMEOUT)
            .map_err(|e| {
                io::Error::other(format!("cp210x: control IN req={:02x}: {}", request, e))
            })?;
        if n != data.len() {
            return Err(io::Error::other(format!(
                "cp210x: control IN req={:02x}: short transfer {}/{}",
                request,
                n,
                data.len()
            )));
        }
        Ok(())
    }
}

impl Port for Cp210xPort {
    fn read(&self, buf: &mut [u8]) -> io::Result<usize> {
        if self.closed.load(Ordering::Acquire) {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "port closed"));
        }
        match self.handle.read_bulk(self.in_endpoint, buf, READ_TIMEOUT) {
            Ok(n) => Ok(n),
            Err(rusb::Error::Timeout) => {
                Err(io::Error::new(io::ErrorKind::TimedOut, "cp210x read timeout"))
            }
            Err(_) if self.closed.load(Ordering::Acquire) => {
                Err(io::Error::new(io::ErrorKind::UnexpectedEof, "port closed"))
            }
            Err(e) => Err(rusb_to_io(e)),
        }
    }

    fn write(&self, buf: &[u8]) -> io::Result<usize> {
        if self.closed.load(Ordering::Acquire) {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "port closed"));
        }
        self.handle
            .write_bulk(self.out_endpoint, buf, WRITE_TIMEOUT)
            .map_err(rusb_to_io)
    }

    fn set_baud_rate(&self, baud: u32) -> io::Result<()> {
        self.set_baud_rate_inner(baud)
    }

    fn set_framing(&self, f: Framing) -> io::Result<()> {
        self.set_framing_inner(f)
    }

    fn set_flow_control(&self, fc: FlowControl) -> io::Result<()> {
        self.set_flow_control_inner(fc)
    }

    fn set_dtr(&self, assert: bool) -> io::Result<()> {
        self.dtr.store(assert, Ordering::Release);
        self.write_mhs(assert, self.rts.load(Ordering::Acquire), true, false)
    }

    fn set_rts(&self, assert: bool) -> io::Result<()> {
        self.rts.store(assert, Ordering::Release);
        self.write_mhs(self.dtr.load(Ordering::Acquire), assert, false, true)
    }

    fn modem_status(&self) -> io::Result<ModemStatus> {
        let mut buf = [0u8; 1];
        self.control_in(REQ_GET_MDM_STS, 0, &mut buf)?;
        let b = buf[0];
        Ok(ModemStatus {
            cts: b & MDM_CTS != 0,
            dsr: b & MDM_DSR != 0,
            ri: b & MDM_RI != 0,
            dcd: b & MDM_DCD != 0,
        })
    }

    fn send_break(&self, duration: Duration) -> io::Result<()> {
        if duration.is_zero() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "cp210x: break duration must be positive",
            ));
        }
        self.control_out(REQ_SET_BREAK, 1, &[])?;
        std::thread::sleep(duration);
        self.control_out(REQ_SET_BREAK, 0, &[])
    }

    fn close(&self) -> io::Result<()> {
        if !self.closed.swap(true, Ordering::AcqRel) {
            let _ = self.set_ifc_enable(false);
            self.teardown();
        }
        Ok(())
    }
}

impl Drop for Cp210xPort {
    fn drop(&mut self) {
        let _ = <Self as Port>::close(self);
    }
}

// --- Helpers ------------------------------------------------------------

/// Re-enumerate the bus and return the handle matching
/// `target`. Serial is used to disambiguate when the device has
/// one; otherwise the first VID/PID match wins (mirroring the Go
/// version's behavior).
fn find_device(target: &Device) -> io::Result<rusb::Device<GlobalContext>> {
    let devices = rusb::devices().map_err(rusb_to_io)?;
    let mut first_match: Option<rusb::Device<GlobalContext>> = None;

    for device in devices.iter() {
        let desc = match device.device_descriptor() {
            Ok(d) => d,
            Err(_) => continue,
        };
        if desc.vendor_id() != target.vendor_id || desc.product_id() != target.product_id {
            continue;
        }

        if target.serial.is_empty() {
            return Ok(device);
        }

        // Serial-disambiguation path — read the descriptor and
        // compare.
        if let Ok(handle) = device.open() {
            let timeout = Duration::from_millis(200);
            if let Ok(langs) = handle.read_languages(timeout) {
                if let Some(lang) = langs.into_iter().next() {
                    if let Some(idx) = desc.serial_number_string_index() {
                        if let Ok(raw) = handle.read_string_descriptor(lang, idx, timeout) {
                            if trim_descriptor(&raw) == target.serial {
                                return Ok(device);
                            }
                        }
                    }
                }
            }
        }
        if first_match.is_none() {
            first_match = Some(device);
        }
    }

    if target.serial.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "cp210x: device {:04x}:{:04x} not found",
                target.vendor_id, target.product_id
            ),
        ));
    }
    match first_match {
        Some(d) => Ok(d),
        None => Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "cp210x: device {:04x}:{:04x} serial {:?} not found",
                target.vendor_id, target.product_id, target.serial
            ),
        )),
    }
}

fn find_bulk_endpoints(iface: &rusb::InterfaceDescriptor) -> io::Result<(u8, u8)> {
    let mut in_ep = None;
    let mut out_ep = None;
    for ep in iface.endpoint_descriptors() {
        if ep.transfer_type() != TransferType::Bulk {
            continue;
        }
        match ep.direction() {
            Direction::In if in_ep.is_none() => in_ep = Some(ep.address()),
            Direction::Out if out_ep.is_none() => out_ep = Some(ep.address()),
            _ => {}
        }
    }
    match (in_ep, out_ep) {
        (Some(in_addr), Some(out_addr)) => Ok((in_addr, out_addr)),
        _ => Err(io::Error::other(
            "cp210x: no bulk IN/OUT endpoint pair on interface",
        )),
    }
}

fn rusb_to_io(err: rusb::Error) -> io::Error {
    let kind = match err {
        rusb::Error::NotFound => io::ErrorKind::NotFound,
        rusb::Error::Access => io::ErrorKind::PermissionDenied,
        rusb::Error::NoDevice => io::ErrorKind::NotFound,
        rusb::Error::Timeout => io::ErrorKind::TimedOut,
        rusb::Error::Busy => io::ErrorKind::WouldBlock,
        rusb::Error::InvalidParam => io::ErrorKind::InvalidInput,
        _ => io::ErrorKind::Other,
    };
    io::Error::new(kind, format!("cp210x: {}", err))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_silabs_stock_vid_pid() {
        let drv = Cp210xDriver;
        assert!(drv.matches(VENDOR_ID, 0xEA60));
        assert!(drv.matches(VENDOR_ID, 0xEA70));
        assert!(!drv.matches(VENDOR_ID, 0x0000));
    }

    #[test]
    fn matches_siemens_rebrand() {
        let drv = Cp210xDriver;
        assert!(drv.matches(0x0908, 0x01FF));
    }

    #[test]
    fn rejects_unrelated_vid() {
        let drv = Cp210xDriver;
        assert!(!drv.matches(0x0403, 0x6001)); // FTDI
    }
}
