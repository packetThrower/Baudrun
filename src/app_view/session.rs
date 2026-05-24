//! Window-creation scaffolding split out of `app_view/mod.rs`:
//!
//!   * [`SessionBundle`] — the moved-between-windows live-session
//!     state ([`AppView::extract_session`] /
//!     [`AppView::install_session`]).
//!   * [`WindowInit`] — discriminator passed to `open_app_window` for
//!     each of the three startup flavours (fresh empty, fresh +
//!     auto-connect to a profile, install a moved session).
//!   * Geometry helpers — `WindowGeometry` ↔ `gpui::Bounds<Pixels>`
//!     conversions used by `open_app_window`'s saved-bounds restore
//!     path and the `cx.on_window_should_close` snapshot.
//!
//! Fields on `SessionBundle` are `pub(super)` because the parent
//! module's `extract_session` / `install_session` methods construct
//! and destructure it directly (the bundle is intentionally a thin
//! container, not a closed type with constructors).

use gpui::{px, Bounds, Entity, Pixels, Task};

use super::{TransferIo, TransferState};
use crate::data::settings;
use crate::serial_io;
use crate::terminal_view::TerminalView;

/// Bundle of live serial-session state that can be moved from one
/// window's `AppView` to another. Captures everything the source
/// AppView held about the live connection — the TerminalView entity
/// (bytes already on screen and in scrollback), the OS-side
/// disconnect token, the read-loop drain task, transfer I/O, and
/// any in-flight transfer + auto-reconnect bookkeeping. Built by
/// [`AppView::extract_session`] on the source side and consumed by
/// [`AppView::install_session`] on the destination side.
pub struct SessionBundle {
    pub(super) terminal: Entity<TerminalView>,
    pub(super) drain_task: Option<Task<()>>,
    pub(super) serial_disconnect: serial_io::Disconnect,
    pub(super) transfer_io: TransferIo,
    pub(super) transfer: Option<TransferState>,
    pub(super) connected_profile_id: String,
    pub(super) auto_reconnect_for: Option<String>,
    pub(super) auto_reconnect_task: Option<Task<()>>,
    pub(super) last_highlight_sig: Option<Vec<(String, String, bool)>>,
}

/// What kind of window to open via [`super::open_app_window`]. `Fresh`
/// takes a caller-built TerminalView (so main.rs can still hold the
/// handle for the CLI serial-port attach path) and lands on the
/// welcome screen. `WithSession` accepts a moved [`SessionBundle`]
/// and installs it after construction so the destination window
/// comes up already connected, with the source window's terminal
/// contents intact. `FreshAutoConnect` opens a fresh window and
/// immediately connects to a named profile — used by the
/// right-click "Connect in New Window" path on the sidebar.
pub enum WindowInit {
    Fresh(Entity<TerminalView>),
    // Boxed to keep `WindowInit`'s stack footprint small —
    // SessionBundle carries the whole live-session state machine
    // (terminal entity, serial channels, transfer state, …) so
    // inlining it bloats the other variants too.
    WithSession(Box<SessionBundle>),
    FreshAutoConnect {
        terminal: Entity<TerminalView>,
        profile_id: String,
    },
}

/// Convert a saved geometry record into the bounds shape
/// `WindowOptions` wants. Returns `None` when the saved record is
/// missing dimensions — caller falls back to the centered default.
pub(super) fn geometry_to_bounds(g: &settings::WindowGeometry) -> Option<Bounds<Pixels>> {
    if g.width <= 0 || g.height <= 0 {
        return None;
    }
    Some(Bounds {
        origin: gpui::point(px(g.x as f32), px(g.y as f32)),
        size: gpui::size(px(g.width as f32), px(g.height as f32)),
    })
}

/// Snapshot the live window bounds into the serializable form used
/// for on-disk persistence. Float pixels round-trip to `i32` since
/// the underlying OS APIs all return integer-pixel rects anyway.
pub(super) fn bounds_to_geometry(b: Bounds<Pixels>) -> settings::WindowGeometry {
    settings::WindowGeometry {
        x: f32::from(b.origin.x) as i32,
        y: f32::from(b.origin.y) as i32,
        width: f32::from(b.size.width) as i32,
        height: f32::from(b.size.height) as i32,
    }
}
