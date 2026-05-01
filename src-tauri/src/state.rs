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
use std::sync::{Arc, Mutex, RwLock};

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

    /// Pending xterm.js buffer snapshots indexed by target window
    /// label. Set during `migrate_session` when the source window
    /// hands us its serialized terminal scrollback; consumed exactly
    /// once on target mount via `take_pending_terminal_snapshot`.
    /// Cleared along with the rest of a window's state when the
    /// window is destroyed.
    pub pending_terminal_snapshots: Mutex<HashMap<String, String>>,

    /// Pending initial-profile ids indexed by spawned-window label.
    /// Set inside `open_profile_window` so the new window's renderer
    /// can pull it on mount via `take_pending_profile_id` and
    /// pre-select that profile. Originally this rode in the spawned
    /// window's URL as `?profile=<id>` but the `?` is an invalid
    /// path character on Windows and the resulting URL never
    /// resolved (blank webview). IPC carries the value cross-platform
    /// without URL encoding gymnastics.
    pub pending_profile_ids: Mutex<HashMap<String, String>>,
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
    /// leave a dangling [`SessionHandle`] in the map. Also drops
    /// any pending terminal snapshot or initial-profile id keyed by
    /// the same label so a window closed before its renderer ever
    /// drained them doesn't leak.
    pub fn forget_session(&self, label: &str) {
        self.sessions.lock().unwrap().remove(label);
        self.pending_terminal_snapshots
            .lock()
            .unwrap()
            .remove(label);
        self.pending_profile_ids.lock().unwrap().remove(label);
    }

    /// Stash a serialized xterm buffer for `target_label` to pick up
    /// on its next mount. Migration sets this from the source's
    /// snapshot; the target's renderer drains it via
    /// [`commands::window::take_pending_terminal_snapshot`].
    pub fn store_pending_terminal_snapshot(&self, target_label: &str, data: String) {
        if data.is_empty() {
            return;
        }
        self.pending_terminal_snapshots
            .lock()
            .unwrap()
            .insert(target_label.to_string(), data);
    }

    pub fn take_pending_terminal_snapshot(&self, label: &str) -> Option<String> {
        self.pending_terminal_snapshots
            .lock()
            .unwrap()
            .remove(label)
    }

    /// Stash an initial profile id for `target_label` to pull on
    /// mount. Called from `open_profile_window` right after the new
    /// window is built so the value is available before the
    /// renderer's onMount completes.
    pub fn store_pending_profile_id(&self, target_label: &str, profile_id: String) {
        if profile_id.is_empty() {
            return;
        }
        self.pending_profile_ids
            .lock()
            .unwrap()
            .insert(target_label.to_string(), profile_id);
    }

    pub fn take_pending_profile_id(&self, label: &str) -> Option<String> {
        self.pending_profile_ids.lock().unwrap().remove(label)
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

    /// Shared cell holding the window label this session's
    /// background callbacks (on_read / on_exit / transfer-progress)
    /// emit events to. The closures read this every fire so the
    /// session's stream can be rerouted to a new window via
    /// `commands::window::migrate_session` without rebuilding the
    /// `Session` itself. `None` when the handle has no active session.
    pub event_target_label: Option<Arc<RwLock<String>>>,
}
