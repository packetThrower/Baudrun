//! App-wide state managed by Tauri. One instance lives in
//! `AppHandle::state()` for the lifetime of the process; commands
//! reach it with `tauri::State<AppState>`.
//!
//! ## Per-window sessions
//!
//! Every Tauri window — the original `main` plus any tear-off windows
//! a user spawns via "Open profile in new window" — gets its own
//! [`SessionHandle`] keyed by window label. Two windows can therefore
//! hold independent serial connections to different ports, run
//! transfers in parallel, etc. Commands route to the correct handle
//! by passing the calling [`tauri::WebviewWindow`] in and locking
//! [`AppState::sessions`] on its label.
//!
//! Note: a single physical port can still only be opened once at the
//! OS level, so two windows can't both connect to the same port.
//! That's enforced naturally by the OS — `Session::open` will fail
//! the second caller with `EBUSY` / equivalent.

use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};

use crate::highlight;
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
    pub highlight: highlight::Store,

    /// Per-window session state. Keyed by Tauri window label
    /// (`"main"` or a `win-<uuid>` slug for spawned windows). Use
    /// [`AppState::with_session`] / [`AppState::with_session_mut`]
    /// instead of locking directly so the lock-time stays short and
    /// the get-or-insert idiom isn't repeated everywhere.
    pub sessions: Mutex<HashMap<String, SessionHandle>>,
}

impl AppState {
    /// Run `f` against the [`SessionHandle`] for `label`, inserting a
    /// default one if the window hasn't accumulated any session
    /// state yet. Holds the sessions lock for the duration of `f` —
    /// callers should keep `f` quick (mutate fields and return; do
    /// not perform I/O inside).
    pub fn with_session<R>(
        &self,
        label: &str,
        f: impl FnOnce(&mut SessionHandle) -> R,
    ) -> R {
        let mut guard = self.sessions.lock().unwrap();
        let handle = guard.entry(label.to_string()).or_default();
        f(handle)
    }

    /// Drop a window's session state entirely. Call from the
    /// `WindowEvent::Destroyed` handler so a closed window doesn't
    /// leave a dangling [`SessionHandle`] in the map.
    pub fn forget_session(&self, label: &str) {
        self.sessions.lock().unwrap().remove(label);
    }
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
