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

/// Broadcast every time the settings store is written (currently
/// from `update_settings`). Every window's renderer subscribes and
/// refreshes its local `settings` store + derived state (skin,
/// appearance) so cross-window edits stay in sync — e.g. changing
/// the skin in the Settings window must repaint the main window
/// immediately. Payload is the full updated [`crate::settings::Settings`].
pub const SETTINGS_UPDATED: &str = "settings:updated";

/// Payload of [`TRANSFER_PROGRESS`].
#[derive(Debug, Clone, Copy, Serialize)]
pub struct TransferProgress {
    pub sent: u64,
    pub total: u64,
}
