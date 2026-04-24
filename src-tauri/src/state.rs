//! App-wide state managed by Tauri. One instance lives in
//! `AppHandle::state()` for the lifetime of the process; commands
//! reach it with `tauri::State<AppState>`.

use std::sync::atomic::AtomicBool;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};

use crate::profiles;
use crate::serial::Session;
use crate::settings;
use crate::skins;
use crate::themes;

pub struct AppState {
    pub profiles: profiles::Store,
    pub settings: settings::Store,
    pub themes: themes::Store,
    pub skins: skins::Store,

    /// The single active serial session (one port at a time — the
    /// hardware can't be opened twice) plus its ancillary state.
    pub session: Mutex<SessionHandle>,
}

#[derive(Default)]
pub struct SessionHandle {
    /// The open session, or `None` when disconnected / pending
    /// reconnect.
    pub session: Option<Arc<Session>>,

    /// Profile id of the active (or last active, during a reconnect
    /// window) session. Empty string when nothing is connected —
    /// matches the Go version's `sessID`.
    pub profile_id: String,

    /// Snapshot of the profile used to open the current session.
    /// Captured at connect time so auto-reconnect can reopen with
    /// the same config even if the user edits the profile mid-
    /// reconnect.
    pub profile_snapshot: Option<profiles::Profile>,

    /// Signal flag for an in-flight auto-reconnect loop. Setting it
    /// to true terminates the loop on its next tick.
    pub reconnect_cancel: Option<Arc<AtomicBool>>,

    /// Sender end of the channel feeding the transfer state machine's
    /// [`crate::transfer::ChannelReader`]. Present only while a
    /// `send_file` command is running.
    pub transfer_tx: Option<mpsc::Sender<Vec<u8>>>,

    /// Cancel flag for the in-flight file transfer. Set by
    /// `cancel_transfer`, observed by `send_xmodem` /
    /// `send_ymodem` between blocks.
    pub transfer_cancel: Option<Arc<AtomicBool>>,
}
