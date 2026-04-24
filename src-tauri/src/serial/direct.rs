//! Stub for the libusb-direct serial backend. The Go version of
//! Baudrun uses the author's `usbserial-go` library to talk to
//! CP210x adapters directly over libusb — useful on macOS when the
//! SiLabs VCP driver isn't installed (or on rebranded devices whose
//! VID doesn't bind to the kernel's id table).
//!
//! No mature Rust equivalent exists. Porting the CP210x control-
//! transfer protocol to `rusb` is a substantial sub-project that
//! falls outside the scope of the Wails → Tauri v2 migration. This
//! module keeps the enumeration + open call sites intact so the rest
//! of the serial layer (and its tests) compile unchanged; when we
//! wire up a Rust CP210x driver later, the stubs become real.
//!
//! See `non-tauri-features.md` for the tracking entry.

use super::ports::PortInfo;

/// List libusb-direct devices. Stub returns empty — rusb integration
/// lands in a post-migration phase.
pub fn list_direct_usb() -> Vec<PortInfo> {
    Vec::new()
}
