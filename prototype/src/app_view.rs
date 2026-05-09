//! AppView — Phase 2's outer window entity. Replaces TerminalView
//! as the window root. Owns a sidebar (profile list, settings
//! affordances later) and the existing TerminalView, laid out as a
//! horizontal split.
//!
//! For this slice the sidebar is read-only display: it lists
//! profiles from `data::profiles::Store` and shows the highlight
//! style of the selected one, but clicking doesn't do anything yet
//! (Phase 2 slice 2 wires connect-by-profile). The TerminalView
//! continues to drive its own serial port + loopback path
//! independently.
//!
//! Built from div primitives rather than `gpui-component`'s
//! `sidebar` widget. The widget is more polished but adds
//! integration surface area; we'll swap it in once the basic
//! layout is proven and the connection-state plumbing is in place.
//! That swap should be mostly mechanical — same structure, fancier
//! divs.
//!
//! No focus tracking on the AppView itself. Focus stays on the
//! TerminalView so keystrokes still reach the grid; sidebar is
//! pointer-driven.

use std::rc::Rc;

use gpui::{
    div, prelude::*, px, rgb, Context, Entity, IntoElement, MouseButton, MouseUpEvent, Render,
    Task, Window,
};
use gpui_component::input::{Input, InputState};

use crate::data::profiles::{self, Profile};
use crate::serial_io;
use crate::terminal_view::TerminalView;

/// Width of the left sidebar in logical pixels. Matches the main
/// app's sidebar width — wide enough for two-line profile rows
/// (name + port_name) without truncation on typical setups, narrow
/// enough that the terminal still gets the lion's share of the
/// window.
const SIDEBAR_WIDTH_PX: f32 = 220.0;

/// Sidebar background colour. Slightly lighter than the terminal's
/// default bg so the split is visually obvious without a border.
const SIDEBAR_BG: u32 = 0x1a1a1e;
/// Sidebar separator (thin vertical line between sidebar and viewport).
const SIDEBAR_BORDER: u32 = 0x2a2a30;
/// Sidebar default text colour.
const SIDEBAR_FG: u32 = 0xd4d4d8;
/// Sidebar muted text colour (port name, section labels).
const SIDEBAR_MUTED: u32 = 0x8a8a92;
/// Highlighted-row background when a profile is selected.
const SIDEBAR_SELECTED: u32 = 0x2d3548;

pub struct AppView {
    terminal: Entity<TerminalView>,
    profile_store: Rc<profiles::Store>,
    /// Most recently clicked profile. Drives row highlight; survives
    /// connect failures so the user can see *which* profile they
    /// just tried (and the inline error text under it).
    selected_profile_id: Option<String>,
    /// Profile whose serial port is currently open and feeding
    /// bytes into the terminal. Distinct from `selected_profile_id`:
    /// a click selects + attempts connect, but a failed open leaves
    /// `selected` set while `connected` stays `None`. Used to paint
    /// the green status dot in the sidebar.
    connected_profile_id: Option<String>,
    /// Last-attempted-connection error for the selected profile,
    /// shown inline in the sidebar row. `Some` only while the
    /// failed profile is still selected; cleared when the user
    /// picks a different profile or the connection later succeeds.
    connect_error: Option<String>,
    /// Foreground async task draining the active connection's read
    /// channel into `TerminalView::feed_bytes`. Held (not detached)
    /// so dropping the field — when switching profiles — also drops
    /// the channel receiver, which lets the OS read thread exit
    /// cleanly. `None` while disconnected (loopback mode).
    drain_task: Option<Task<()>>,
    /// `Some` while the new-profile form is open in the right pane.
    /// The presence of this field also drives a render branch:
    /// when populated the form replaces the TerminalView; when
    /// `None` the terminal is back. Holds `Entity<InputState>`s
    /// per field so the Input widgets persist their text + cursor
    /// across re-renders without us mirroring it into AppView.
    editor: Option<Editor>,
}

/// In-flight new-profile form state. Created by `open_editor` (which
/// needs `&mut Window` because gpui-component's `InputState::new`
/// hooks the window's text-system at construction). Read by
/// `save_editor` to materialize a `Profile`. Dropped on cancel /
/// successful save by setting `AppView::editor = None`.
struct Editor {
    name: Entity<InputState>,
    port: Entity<InputState>,
    baud: Entity<InputState>,
    /// Most recent validation/persistence error. Cleared when the
    /// editor is reopened; populated when `Store::create` rejects
    /// the form values (e.g. blank name, blank port, non-numeric
    /// baud).
    error: Option<String>,
}

impl AppView {
    pub fn new(
        terminal: Entity<TerminalView>,
        profile_store: Rc<profiles::Store>,
        _cx: &mut Context<Self>,
    ) -> Self {
        Self {
            terminal,
            profile_store,
            selected_profile_id: None,
            connected_profile_id: None,
            connect_error: None,
            drain_task: None,
            editor: None,
        }
    }

    /// Click handler for a profile row. Selects + attempts to
    /// connect in one step. Selecting a profile that's already
    /// active is a no-op (we don't want to drop and re-open the
    /// same port on every click). On open failure the profile
    /// stays selected and the error string surfaces in the
    /// sidebar — Phase 2.3 will turn this into a proper
    /// status indicator.
    fn select_profile(&mut self, id: String, _window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_profile_id.as_deref() == Some(id.as_str()) {
            return;
        }
        self.selected_profile_id = Some(id.clone());
        self.connect_error = None;
        let Some(profile) = self.profile_store.get(&id) else {
            self.connect_error = Some("profile not found".into());
            cx.notify();
            return;
        };
        self.connect_to(profile, cx);
        cx.notify();
    }

    /// Disconnect the current session (if any) and open the new
    /// profile's port. The disconnect step is implicit: dropping
    /// `drain_task` drops the receiver, dropping the
    /// `TerminalView::serial_tx` drops the sender — both ends
    /// gone, the OS read/write threads in `serial_io` exit
    /// cleanly because their channels return errors.
    fn connect_to(&mut self, profile: Profile, cx: &mut Context<Self>) {
        // Tear down the previous connection. Order matters less
        // than completeness; both ends must drop for the threads
        // to wind up.
        self.drain_task = None;
        self.connected_profile_id = None;
        self.terminal.update(cx, |t, _| t.clear_serial_tx());

        let port = profile.port_name.clone();
        if port.is_empty() {
            self.connect_error = Some("profile has no port set".into());
            return;
        }
        // `Profile::baud_rate` is `i32` to round-trip via JSON
        // without forcing unsigned. `serial_io::open` wants `u32`;
        // a negative or absurdly-large baud is meaningless here so
        // clamp at zero and accept the truncation. Real baud rates
        // top out at 4M on typical adapters, well within u32.
        let baud = profile.baud_rate.max(0) as u32;

        let channels = match serial_io::open(&port, baud) {
            Ok(c) => c,
            Err(e) => {
                self.connect_error = Some(format!("open {port}: {e}"));
                return;
            }
        };

        log::info!("connected to {port} at {baud} 8N1 (profile {})", profile.id);

        // Wire the write channel into the TerminalView so typing
        // routes to the device.
        let write_tx = channels.write_tx;
        self.terminal.update(cx, |t, _| t.set_serial_tx(write_tx));

        // Spawn the read drain. Held in `drain_task` so a
        // subsequent connect cancels this one by dropping the
        // task field.
        let weak_terminal = self.terminal.downgrade();
        let read_rx = channels.read_rx;
        let task = cx.spawn(async move |_, cx| {
            while let Ok(bytes) = read_rx.recv_async().await {
                if weak_terminal
                    .update(cx, |t, cx| t.feed_bytes(&bytes, cx))
                    .is_err()
                {
                    break;
                }
            }
        });
        self.drain_task = Some(task);
        self.connected_profile_id = Some(profile.id);
    }

    /// Open the new-profile form. Idempotent if already open
    /// (re-creates the InputStates, so the user gets a fresh
    /// blank form rather than whatever they typed before — Phase
    /// 2.4 doesn't need draft persistence).
    fn open_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let name = cx.new(|cx| InputState::new(window, cx).placeholder("My switch"));
        let port =
            cx.new(|cx| InputState::new(window, cx).placeholder("/dev/cu.usbserial-XXX or COM3"));
        let baud = cx.new(|cx| InputState::new(window, cx).default_value("9600"));
        self.editor = Some(Editor {
            name,
            port,
            baud,
            error: None,
        });
        cx.notify();
    }

    fn cancel_editor(&mut self, cx: &mut Context<Self>) {
        self.editor = None;
        cx.notify();
    }

    /// Pull text out of the form, build a `Profile` (other fields
    /// from `Profile::defaults()` — 8N1, no flow control, CR line
    /// ending), persist via the store. On success the form closes;
    /// on validation failure the inline error is set and the form
    /// stays open so the user can fix it.
    fn save_editor(&mut self, cx: &mut Context<Self>) {
        let Some(editor) = self.editor.as_ref() else {
            return;
        };
        let name = editor.name.read(cx).value().to_string();
        let port = editor.port.read(cx).value().to_string();
        let baud_str = editor.baud.read(cx).value().to_string();
        // Empty / non-numeric → 0, which `validate` rejects with
        // `InvalidBaud`; let the store be the single source of
        // truth on what counts as valid rather than duplicating
        // its rules in the UI.
        let baud: i32 = baud_str.trim().parse().unwrap_or(0);

        let mut profile = Profile::defaults();
        profile.name = name;
        profile.port_name = port;
        profile.baud_rate = baud;

        match self.profile_store.create(profile) {
            Ok(_) => {
                self.editor = None;
            }
            Err(e) => {
                if let Some(ed) = self.editor.as_mut() {
                    ed.error = Some(format!("{e}"));
                }
            }
        }
        cx.notify();
    }
}

impl Render for AppView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let profiles = self.profile_store.list();
        let selected = self.selected_profile_id.clone();
        let connected = self.connected_profile_id.clone();

        // Right pane: form when an editor is open, terminal otherwise.
        // Branching here (instead of conditionally adding children to
        // the same div) lets each branch pick its own padding /
        // background without leaking into the other.
        let right_pane: gpui::AnyElement = match self.editor.as_ref() {
            Some(editor) => form_pane(
                editor.name.clone(),
                editor.port.clone(),
                editor.baud.clone(),
                editor.error.clone(),
                cx,
            )
            .into_any_element(),
            None => div()
                .flex_1()
                .h_full()
                .child(self.terminal.clone())
                .into_any_element(),
        };

        div()
            .size_full()
            .flex()
            .flex_row()
            .bg(rgb(SIDEBAR_BG))
            // -- sidebar --
            .child(
                div()
                    .w(px(SIDEBAR_WIDTH_PX))
                    .h_full()
                    .bg(rgb(SIDEBAR_BG))
                    .border_r_1()
                    .border_color(rgb(SIDEBAR_BORDER))
                    .px_2()
                    .py_3()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .text_color(rgb(SIDEBAR_FG))
                    .text_size(px(13.0))
                    .font_family("Menlo")
                    .child(sidebar_header(cx))
                    .children(profiles.into_iter().map(|profile| {
                        let is_selected = selected.as_deref() == Some(profile.id.as_str());
                        let is_connected = connected.as_deref() == Some(profile.id.as_str());
                        let row_error = if is_selected {
                            self.connect_error.clone()
                        } else {
                            None
                        };
                        // Connected wins over Failed when both apply
                        // (shouldn't happen — connect_to clears the
                        // error before setting connected — but defining
                        // the precedence keeps the indicator stable
                        // if that invariant ever drifts).
                        let status = if is_connected {
                            Some(RowStatus::Connected)
                        } else if is_selected && row_error.is_some() {
                            Some(RowStatus::Failed)
                        } else {
                            None
                        };
                        profile_row(profile, is_selected, status, row_error, cx)
                    })),
            )
            // -- right pane: form OR terminal --
            .child(right_pane)
    }
}

/// Sidebar header row: muted "PROFILES" label on the left, "+"
/// affordance on the right that opens the new-profile form. The
/// "+" is a div-with-click rather than a real button widget — same
/// reasoning as the rest of the sidebar (less surface area than
/// adopting `gpui_component::button` for one element).
fn sidebar_header(cx: &mut Context<AppView>) -> impl IntoElement {
    div()
        .w_full()
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .py_1()
        .child(
            div()
                .text_size(px(11.0))
                .text_color(rgb(SIDEBAR_MUTED))
                .child("PROFILES"),
        )
        .child(
            div()
                .px_2()
                .text_size(px(16.0))
                .text_color(rgb(SIDEBAR_FG))
                .rounded_sm()
                .hover(|s| s.bg(rgb(SIDEBAR_SELECTED)))
                .cursor_pointer()
                .child("+")
                .on_mouse_up(
                    MouseButton::Left,
                    cx.listener(|this, _: &MouseUpEvent, window, cx| {
                        this.open_editor(window, cx);
                    }),
                ),
        )
}

/// Background colour for the form pane. Matches the terminal's
/// default background so the right-pane swap doesn't flash a
/// different shade — visually it's the same canvas with different
/// content.
const FORM_BG: u32 = 0x0b0b0d;
/// Save-button green. Same hue as the connected status dot so the
/// "submit and connect" affordance reads as the positive action.
const SAVE_BTN_BG: u32 = 0x4ade80;
/// Save-button text colour. Dark on green for contrast.
const SAVE_BTN_FG: u32 = 0x0b0b0d;

fn form_pane(
    name: Entity<InputState>,
    port: Entity<InputState>,
    baud: Entity<InputState>,
    error: Option<String>,
    cx: &mut Context<AppView>,
) -> impl IntoElement {
    let labeled = |label: &'static str, input: Entity<InputState>| {
        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(
                div()
                    .text_size(px(11.0))
                    .text_color(rgb(SIDEBAR_MUTED))
                    .child(label),
            )
            .child(Input::new(&input).appearance(true))
    };

    div()
        .flex_1()
        .h_full()
        .bg(rgb(FORM_BG))
        .px_8()
        .py_6()
        .flex()
        .flex_col()
        .gap_4()
        .text_color(rgb(SIDEBAR_FG))
        .text_size(px(13.0))
        .font_family("Menlo")
        .child(div().text_size(px(16.0)).child("New profile"))
        .child(labeled("NAME", name))
        .child(labeled("PORT", port))
        .child(labeled("BAUD", baud))
        .child(
            div()
                .flex()
                .flex_row()
                .gap_2()
                .mt_4()
                .child(
                    div()
                        .px_4()
                        .py_2()
                        .bg(rgb(SAVE_BTN_BG))
                        .text_color(rgb(SAVE_BTN_FG))
                        .rounded_sm()
                        .cursor_pointer()
                        .child("Save")
                        .on_mouse_up(
                            MouseButton::Left,
                            cx.listener(|this, _: &MouseUpEvent, _window, cx| {
                                this.save_editor(cx);
                            }),
                        ),
                )
                .child(
                    div()
                        .px_4()
                        .py_2()
                        .bg(rgb(SIDEBAR_SELECTED))
                        .text_color(rgb(SIDEBAR_FG))
                        .rounded_sm()
                        .cursor_pointer()
                        .child("Cancel")
                        .on_mouse_up(
                            MouseButton::Left,
                            cx.listener(|this, _: &MouseUpEvent, _window, cx| {
                                this.cancel_editor(cx);
                            }),
                        ),
                ),
        )
        .children(error.map(|err| {
            div()
                .text_size(px(12.0))
                .text_color(rgb(SIDEBAR_ERROR))
                .child(err)
        }))
}

/// Bright reddish-pink for the inline connect-error message
/// under a profile row. Bright enough to read on the dark sidebar
/// without being painful.
const SIDEBAR_ERROR: u32 = 0xff7a8a;
/// Live-connection status dot colour. Saturated green that reads
/// at 8px against the dark sidebar without being neon.
const STATUS_CONNECTED: u32 = 0x4ade80;
/// Failed-connection status dot colour. Reuses the inline-error
/// pink so the dot and the under-row error text agree visually.
const STATUS_FAILED: u32 = SIDEBAR_ERROR;
/// Diameter of the round status dot in the row header. 8px reads
/// at the sidebar's font size without crowding the name text.
const STATUS_DOT_PX: f32 = 8.0;

/// Per-row connection state used to paint the status dot. `None`
/// (in the caller) means no dot at all; an explicit `Connected`/
/// `Failed` keeps the dot's two cases tagged so the row colour
/// table stays a one-line lookup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RowStatus {
    Connected,
    Failed,
}

impl RowStatus {
    fn color(self) -> u32 {
        match self {
            RowStatus::Connected => STATUS_CONNECTED,
            RowStatus::Failed => STATUS_FAILED,
        }
    }
}

fn profile_row(
    profile: Profile,
    is_selected: bool,
    status: Option<RowStatus>,
    error: Option<String>,
    cx: &mut Context<AppView>,
) -> impl IntoElement {
    let id = profile.id.clone();
    let name = if profile.name.is_empty() {
        "(unnamed)".to_string()
    } else {
        profile.name.clone()
    };
    let port = if profile.port_name.is_empty() {
        "no port set".to_string()
    } else {
        profile.port_name.clone()
    };

    let bg = if is_selected {
        rgb(SIDEBAR_SELECTED)
    } else {
        rgb(SIDEBAR_BG)
    };

    // Header row: name on the left, status dot on the right (only
    // when there's a status to show — `None` collapses the slot so
    // unstatussed rows don't reserve space for an absent dot).
    let header = div()
        .w_full()
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .child(div().text_color(rgb(SIDEBAR_FG)).child(name))
        .children(status.map(|s| {
            div()
                .w(px(STATUS_DOT_PX))
                .h(px(STATUS_DOT_PX))
                .rounded_full()
                .bg(rgb(s.color()))
        }));

    let mut row = div()
        .w_full()
        .px_2()
        .py_1()
        .rounded_sm()
        .bg(bg)
        .hover(|s| s.bg(rgb(SIDEBAR_SELECTED)))
        .cursor_pointer()
        .flex()
        .flex_col()
        .gap_1()
        .child(header)
        .child(
            div()
                .text_size(px(11.0))
                .text_color(rgb(SIDEBAR_MUTED))
                .child(port),
        );
    if let Some(err) = error {
        row = row.child(
            div()
                .text_size(px(11.0))
                .text_color(rgb(SIDEBAR_ERROR))
                .child(err),
        );
    }
    row.on_mouse_up(
        MouseButton::Left,
        cx.listener(move |this, _: &MouseUpEvent, window, cx| {
            this.select_profile(id.clone(), window, cx);
        }),
    )
}
