//! Event names and payload types emitted on the Tauri event bus.
//! Kept as constants so backend emitters and frontend listeners can
//! both reference the same symbols without drift.

use serde::Serialize;

pub const SERIAL_DATA: &str = "serial:data";
pub const SERIAL_DISCONNECT: &str = "serial:disconnect";
pub const SERIAL_RECONNECTING: &str = "serial:reconnecting";
pub const SERIAL_RECONNECTED: &str = "serial:reconnected";

pub const TRANSFER_PROGRESS: &str = "transfer:progress";
pub const TRANSFER_COMPLETE: &str = "transfer:complete";
pub const TRANSFER_ERROR: &str = "transfer:error";

/// Payload of [`TRANSFER_PROGRESS`].
#[derive(Debug, Clone, Copy, Serialize)]
pub struct TransferProgress {
    pub sent: u64,
    pub total: u64,
}
