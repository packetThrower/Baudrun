//! Serial layer — port enumeration, session lifecycle, USB chipset
//! detection, and the platform-specific "missing driver" sniffers.
//! Mirrors the architecture of the Go `internal/serial` package: one
//! backend trait, a session-owned read pump, and per-OS modules for
//! detection paths that need native tools (ioreg on macOS,
//! Get-PnpDevice on Windows).

pub mod chipsets;
// detect.rs is only consumed by usb_darwin / usb_windows; on Linux
// the missing-driver enumeration is a stub (usb_other.rs) so the
// suspect-port helper would be dead code there. Gating the module
// keeps clippy -D warnings happy.
#[cfg(any(target_os = "macos", target_os = "windows"))]
pub mod detect;
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
// Re-exports from the Tauri-era serial layer. The gpui code in
// `src/serial_io.rs` uses `serialport` directly, so these names
// have no in-crate caller — narrowed allow preserves the public
// API surface (in case the libusb path comes back) without
// suppressing accidental orphans elsewhere.
#[allow(unused_imports)]
pub use ports::{list_ports, PortInfo};
#[allow(unused_imports)]
pub use session::{Config, ControlLines, OnExit, OnRead, Session, SessionError, TransferSink};

/// USB devices whose VID matches a known serial chipset but which
/// aren't currently accessible as serial ports — i.e. the user
/// probably hasn't installed the vendor driver. Implementation lives
/// in the platform-specific module selected at build time.
#[allow(dead_code)]
pub fn detect_missing_drivers() -> Result<Vec<USBSerialCandidate>, String> {
    usb_platform::detect_missing_drivers()
}
