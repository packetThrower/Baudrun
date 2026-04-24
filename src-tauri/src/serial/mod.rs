//! Serial layer — port enumeration, session lifecycle, USB chipset
//! detection, and the platform-specific "missing driver" sniffers.
//! Mirrors the architecture of the Go `internal/serial` package: one
//! backend trait, a session-owned read pump, and per-OS modules for
//! detection paths that need native tools (ioreg on macOS,
//! Get-PnpDevice on Windows).

pub mod chipsets;
mod detect;
mod direct;
pub mod ports;
pub mod session;

#[cfg(target_os = "macos")]
#[path = "usb_darwin.rs"]
mod usb_platform;

#[cfg(target_os = "windows")]
#[path = "usb_windows.rs"]
mod usb_platform;

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
#[path = "usb_other.rs"]
mod usb_platform;

pub use chipsets::USBSerialCandidate;
pub use ports::{list_ports, PortInfo};
pub use session::{Config, ControlLines, OnExit, OnRead, Session, SessionError, TransferSink};

/// USB devices whose VID matches a known serial chipset but which
/// aren't currently accessible as serial ports — i.e. the user
/// probably hasn't installed the vendor driver. Implementation lives
/// in the platform-specific module selected at build time.
pub fn detect_missing_drivers() -> Result<Vec<USBSerialCandidate>, String> {
    usb_platform::detect_missing_drivers()
}
