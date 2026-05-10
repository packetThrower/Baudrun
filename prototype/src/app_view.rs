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

use std::fs::File;
use std::io::Write;
use std::rc::Rc;

use std::time::Duration;

use gpui::{
    div, prelude::*, pulsating_between, px, rgb, rgba, Animation, AnimationExt, AppContext, Bounds,
    Context, Entity, IntoElement, MouseButton, MouseUpEvent, Render, ScrollHandle, Task,
    TitlebarOptions, Window, WindowBounds, WindowHandle, WindowOptions,
};
use gpui_component::{
    checkbox::Checkbox,
    input::{Input, InputState},
    scroll::ScrollableElement,
    select::{Select, SelectItem, SelectState},
    IndexPath, Root, Sizable,
};
use gpui::SharedString;

use crate::data::appdata;
use crate::data::highlight;
use crate::data::profiles::{self, Profile};
use crate::data::sanitize::SanitizingLogWriter;
use crate::data::serial::ports;
use crate::data::settings;
use crate::data::skins;
use crate::data::themes;
use crate::settings_bus::{SettingsBus, SettingsEvent};
use crate::settings_view::SettingsView;
use crate::term_bridge::Palette;
use crate::serial_io;
use crate::terminal_view::{ProfileSettings, TerminalView};

/// Width of the left sidebar in logical pixels. Matches the main
/// app's sidebar width — wide enough for two-line profile rows
/// (name + port_name) without truncation on typical setups, narrow
/// enough that the terminal still gets the lion's share of the
/// window.
const SIDEBAR_WIDTH_PX: f32 = 220.0;

/// Sidebar background — `--bg-sidebar` (4% white) composited over
/// the Baudrun `--shell-bg` (#1d1d1e) and baked opaque. Slightly
/// lighter than the form pane on the right, matching the Tauri
/// version's layering.
const SIDEBAR_BG: u32 = 0x262627;
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
    /// Observable settings entity. Wraps the on-disk Store and
    /// emits `SettingsEvent::Updated` after a successful save —
    /// AppView subscribes in `new()` so edits made from the
    /// standalone Settings window trigger an immediate re-render
    /// here. Cloned into SettingsView's open-window builder so the
    /// two windows share the same Entity (gpui entities live at
    /// App scope, not per-window).
    settings_bus: Entity<SettingsBus>,
    /// Held to keep the settings subscription alive for AppView's
    /// lifetime. Drop = unsubscribe.
    _settings_sub: gpui::Subscription,
    /// Skin store (built-in + user). Cloned into the SettingsView
    /// so the Appearance tab can enumerate all skins for its
    /// picker. AppView holds its own handle for the future live-
    /// apply path (settings.skin_id changes → main window
    /// re-renders with the new palette).
    skins_store: Rc<skins::Store>,
    /// Highlight pack store (bundled vendor rules + the editable
    /// user pack + imported third-party packs). Cloned into
    /// SettingsView so the Highlighting tab can enumerate every
    /// pack for its toggle list. Future slices read this from the
    /// terminal pane to apply highlighting to live output.
    #[allow(dead_code)]
    highlight_store: Rc<highlight::Store>,
    /// Terminal theme store (built-in palettes + user imports).
    /// Cloned into SettingsView for the Themes tab picker. Future
    /// slices feed this into `term_bridge::resolve` so a session
    /// picks up the picked palette instead of the hardcoded one.
    #[allow(dead_code)]
    themes_store: Rc<themes::Store>,
    /// Handle to the standalone Settings window when it's open.
    /// `Some(_)` doesn't strictly mean "still alive" — the user may
    /// have closed the OS window. We probe with `handle.update(...)`
    /// before reusing; on `Err` we treat it as gone and open a new
    /// one. Storing the handle lets a second click on the gear
    /// focus the existing Settings window instead of stacking
    /// duplicates (matches the Tauri behaviour).
    settings_window: Option<WindowHandle<Root>>,
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
    /// JoinHandles for the active session's serial read/write
    /// threads. Held so we can `disconnect.wait()` before opening
    /// a new port — otherwise the prior session's threads still
    /// hold the file descriptor and `serialport`'s `TIOCEXCL` makes
    /// the next `open` fail with "Unable to acquire exclusive lock"
    /// (race observed on macOS when toggling profiles quickly).
    /// `None` while disconnected.
    serial_disconnect: Option<serial_io::Disconnect>,
    /// Polling task that retries `connect_to` after an unexpected
    /// session drop. Held so a user disconnect / profile switch
    /// can cancel pending retries by dropping the field. `None`
    /// when not actively trying to reconnect.
    auto_reconnect_task: Option<Task<()>>,
    /// `Some(profile_id)` while auto-reconnect is in flight. Kept
    /// separate from `connected_profile_id` (which only tracks a
    /// live, byte-flowing session) so the right pane can keep the
    /// terminal viewport visible during the retry window — without
    /// this, every reconnect attempt would flicker the user back
    /// to the welcome screen between drops.
    auto_reconnect_for: Option<String>,
    /// `Some` while the new-profile form is open in the right pane.
    /// The presence of this field also drives a render branch:
    /// when populated the form replaces the TerminalView; when
    /// `None` the terminal is back. Holds `Entity<InputState>`s
    /// per field so the Input widgets persist their text + cursor
    /// across re-renders without us mirroring it into AppView.
    editor: Option<Editor>,
}

/// In-flight profile form state. Created by `open_editor` (new) or
/// `open_editor_for` (existing). Both paths need `&mut Window`
/// because gpui-component's `InputState::new` hooks the window's
/// text-system at construction. Read by `save_editor` to materialize
/// a `Profile`. Dropped on cancel / successful save / successful
/// delete by setting `AppView::editor = None`.
/// Which sub-tab inside the form is currently active. Mirrors the
/// Tauri ProfileForm.svelte's left-rail. `Highlighting` is omitted
/// for now — that's a separate phase per user direction.
#[derive(Clone, Copy, PartialEq, Eq)]
enum EditorTab {
    Connection,
    Advanced,
}


struct Editor {
    /// `None` = creating a brand-new profile (Save → `Store::create`).
    /// `Some(id)` = editing an existing one (Save → `Store::update`,
    /// Delete → `Store::delete`). Distinguishing here lets the same
    /// form pane drive both operations without a parallel widget tree.
    profile_id: Option<String>,
    /// Active sub-tab. Default `Connection` on open.
    tab: EditorTab,

    // -- text input --
    name: Entity<InputState>,

    // -- Connection section selects --
    /// Serial Port select. Options come from
    /// `data::serial::ports::list_ports`; if the saved profile's
    /// port isn't currently detected, it's added as a "(not
    /// connected)" option so the form still shows it.
    port: Entity<SelectState<Vec<Opt>>>,
    baud: Entity<SelectState<Vec<Opt>>>,
    data_bits: Entity<SelectState<Vec<Opt>>>,
    parity: Entity<SelectState<Vec<Opt>>>,
    stop_bits: Entity<SelectState<Vec<Opt>>>,
    flow_control: Entity<SelectState<Vec<Opt>>>,

    // -- Terminal section selects + bool --
    line_ending: Entity<SelectState<Vec<Opt>>>,
    backspace_key: Entity<SelectState<Vec<Opt>>>,
    /// Local-echo checkbox state. Stored as a plain `bool` (not an
    /// entity) because gpui-component's `Checkbox` is stateless and
    /// reports changes via callback — the callback writes back here.
    local_echo: bool,

    // -- Advanced / Control Lines selects --
    dtr_on_connect: Entity<SelectState<Vec<Opt>>>,
    rts_on_connect: Entity<SelectState<Vec<Opt>>>,
    dtr_on_disconnect: Entity<SelectState<Vec<Opt>>>,
    rts_on_disconnect: Entity<SelectState<Vec<Opt>>>,

    // -- Advanced / Output bools --
    hex_view: bool,
    timestamps: bool,
    log_enabled: bool,
    auto_reconnect: bool,

    // -- Advanced / Paste safety --
    paste_warn_multiline: bool,
    paste_slow: bool,
    /// Slow-paste delay in ms. Backed by an Input rather than a
    /// number widget — gpui-component doesn't ship one, and keeping
    /// this as a free-form text input lets the store's validator
    /// (which already accepts any non-negative i32) be the single
    /// source of truth.
    paste_char_delay_ms: Entity<InputState>,

    /// Most recent validation/persistence error. Cleared when the
    /// editor is reopened; populated when `Store::create` /
    /// `Store::update` / `Store::delete` rejects the operation
    /// (e.g. blank name, blank port, non-numeric baud).
    error: Option<String>,

    /// Scroll position of the form body. Held on `Editor` so the
    /// form keeps its scroll position across re-renders, and so
    /// the gpui-component `Scrollbar` widget can be wired to the
    /// same handle the scroll content tracks.
    scroll_handle: ScrollHandle,

    /// Snapshot of the profile state the form was loaded from
    /// (and the state any successful Save resets it back to).
    /// Used to compute `is_dirty` in render: derive the current
    /// Profile from widget values, compare against this baseline,
    /// any field difference flags the form as dirty. Cheaper than
    /// wiring a per-widget change subscription, and self-correcting
    /// — Cancel and Connect both restore to baseline implicitly
    /// (editor closes; new open seeds a fresh baseline).
    baseline: Profile,
}

impl AppView {
    pub fn new(
        terminal: Entity<TerminalView>,
        profile_store: Rc<profiles::Store>,
        settings_bus: Entity<SettingsBus>,
        skins_store: Rc<skins::Store>,
        highlight_store: Rc<highlight::Store>,
        themes_store: Rc<themes::Store>,
        cx: &mut Context<Self>,
    ) -> Self {
        // Re-render the main window when the standalone Settings
        // window persists a change, AND apply theme/font-size/etc.
        // edits to the live terminal pane. Future Phase-4 slices
        // hang skin chrome refresh off the same hook.
        let settings_sub = cx.subscribe(
            &settings_bus,
            |this, _, event: &SettingsEvent, cx| {
                let SettingsEvent::Updated(next) = event;
                this.apply_settings(next, cx);
                cx.notify();
            },
        );
        // Apply the persisted theme right away so a fresh launch
        // honours the user's `default_theme_id` instead of paying
        // a one-frame flash of the boot Baudrun palette.
        let initial = settings_bus.read(cx).current().clone();
        let mut this = Self {
            terminal,
            profile_store,
            settings_bus,
            _settings_sub: settings_sub,
            skins_store,
            highlight_store,
            themes_store,
            settings_window: None,
            selected_profile_id: None,
            connected_profile_id: None,
            connect_error: None,
            drain_task: None,
            serial_disconnect: None,
            auto_reconnect_task: None,
            auto_reconnect_for: None,
            editor: None,
        };
        this.apply_settings(&initial, cx);
        this
    }

    /// Apply the relevant slots of a `Settings` snapshot to the
    /// live UI. Called both at construction time and on every
    /// `SettingsEvent::Updated` from the bus. Today: looks up the
    /// active theme via `themes_store` and pushes the resulting
    /// palette to the terminal pane. Future Phase-4 slices add
    /// font-size + skin-token application here.
    fn apply_settings(&mut self, settings: &settings::Settings, cx: &mut Context<Self>) {
        // `default_theme_id` empty falls back to the built-in
        // Baudrun theme — same precedence the Tauri reader uses.
        let theme_id = if settings.default_theme_id.is_empty() {
            themes::DEFAULT_THEME_ID
        } else {
            settings.default_theme_id.as_str()
        };
        let palette = match self.themes_store.get(theme_id) {
            Some(theme) => Palette::from_theme(&theme),
            None => {
                log::warn!("theme {theme_id:?} not found, using built-in baudrun");
                Palette::baudrun()
            }
        };
        self.terminal
            .update(cx, |term, cx| term.set_palette(palette, cx));
    }

    /// Click handler for a profile row. Opens the profile editor
    /// — does NOT auto-connect. The user reaches a live session
    /// only by hitting the Connect button inside the editor;
    /// disconnecting reopens this same editor view (see
    /// `disconnect_current`). This three-state flow (idle ↔
    /// editor ↔ terminal) is the same shape as the Tauri version
    /// and gives the eventual Suspend feature a place to slot in
    /// between editor and terminal.
    fn select_profile(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
        // Clicking the row for the currently-connected profile
        // snaps the view back to its terminal — closes any
        // editor that might be open for a *different* profile.
        // The editor is only reachable via Disconnect from a
        // live session, so clicking the connected row shouldn't
        // yank focus into a form. Clicking ANY OTHER row opens
        // its editor (the active session keeps running in the
        // background).
        if self.connected_profile_id.as_deref() == Some(id.as_str()) {
            if self.editor.is_some() {
                self.editor = None;
                cx.notify();
            }
            // Even when the editor was already closed, clicking
            // the connected row should re-grab focus for the
            // viewport — otherwise typing a Cmd-Tab away and back
            // requires an extra click on the terminal first.
            let viewport_focus = self.terminal.read(cx).focus_handle().clone();
            viewport_focus.focus(window, cx);
            return;
        }
        self.selected_profile_id = Some(id.clone());
        self.connect_error = None;
        // Suppress the open-editor reload if THIS is the profile
        // already in the form — clicking the same row twice
        // shouldn't blow away in-flight edits the user hasn't
        // saved yet.
        if let Some(ed) = self.editor.as_ref() {
            if ed.profile_id.as_deref() == Some(id.as_str()) {
                cx.notify();
                return;
            }
        }
        self.open_editor_for(id, window, cx);
    }

    /// Disconnect the current session (if any) and open the new
    /// profile's port. The disconnect step is implicit: dropping
    /// `drain_task` drops the receiver, dropping the
    /// `TerminalView::serial_tx` drops the sender — both ends
    /// gone, the OS read/write threads in `serial_io` exit
    /// cleanly because their channels return errors.
    fn connect_to(&mut self, profile: Profile, cx: &mut Context<Self>) {
        // Tear down the previous connection FULLY before opening a
        // new one. Order matters: drop the channel ends FIRST so
        // the OS threads notice and start exiting; THEN join their
        // handles via `disconnect.wait()` to wait until they've
        // actually released the port file descriptor. Without the
        // join, the next `serial_io::open` races the prior session's
        // close and fails with "Unable to acquire exclusive lock"
        // when both sides target the same port (macOS TIOCEXCL).
        self.drain_task = None;
        self.connected_profile_id = None;
        // Note: we deliberately do NOT clear `auto_reconnect_task`
        // here. `connect_to` is itself called by the retry loop
        // inside the auto-reconnect Task; clearing the field would
        // drop the Task we're running inside, which gpui interprets
        // as a cancel signal and the loop terminates after the
        // current iteration. User-initiated paths (save_and_connect,
        // disconnect_current, delete_from_editor) clear the field
        // explicitly so retries for other profiles still get
        // cancelled when the user picks a different one.
        self.terminal.update(cx, |t, _| t.clear_serial_tx());
        // Flag the prior session's threads to exit. Returns
        // immediately; threads release the port file descriptor
        // within ~SHUTDOWN_POLL_INTERVAL. The retry loop below
        // covers that gap — we don't synchronously join because a
        // stuck `port.write_all` would freeze the UI.
        if let Some(d) = self.serial_disconnect.take() {
            d.shutdown();
        }

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

        let policies = serial_io::LinePolicies {
            dtr_on_connect: serial_io::LinePolicy::from_str(&profile.dtr_on_connect),
            rts_on_connect: serial_io::LinePolicy::from_str(&profile.rts_on_connect),
            dtr_on_disconnect: serial_io::LinePolicy::from_str(&profile.dtr_on_disconnect),
            rts_on_disconnect: serial_io::LinePolicy::from_str(&profile.rts_on_disconnect),
        };
        // Retry briefly: a fresh disconnect may have left the
        // prior session's threads still holding the port file
        // descriptor. Each attempt waits 30ms before retrying;
        // 5 attempts × 30ms = max ~150ms blocking on the UI
        // thread, which beats the unbounded freeze of synchronous
        // join. `LinePolicies` is `Copy` so re-passing it is free.
        let channels = {
            let mut last_err = None;
            let mut got = None;
            for attempt in 0..5 {
                match serial_io::open(&port, baud, policies) {
                    Ok(c) => {
                        got = Some(c);
                        break;
                    }
                    Err(e) => {
                        if attempt < 4 {
                            std::thread::sleep(std::time::Duration::from_millis(30));
                        }
                        last_err = Some(e);
                    }
                }
            }
            match got {
                Some(c) => c,
                None => {
                    let msg = last_err
                        .map(|e| e.to_string())
                        .unwrap_or_else(|| "unknown".into());
                    self.connect_error = Some(format!("open {port}: {msg}"));
                    return;
                }
            }
        };

        log::info!("connected to {port} at {baud} 8N1 (profile {})", profile.id);

        // Wire the write channel into the TerminalView so typing
        // routes to the device, and push the profile-driven
        // keystroke settings (line_ending, backspace_key,
        // local_echo) so the encoder honours them on the next
        // keypress.
        let write_tx = channels.write_tx;
        let settings = ProfileSettings {
            line_ending: profile.line_ending.clone(),
            backspace_key: profile.backspace_key.clone(),
            local_echo: profile.local_echo,
            paste_warn_multiline: profile.paste_warn_multiline,
            paste_slow: profile.paste_slow,
            paste_char_delay_ms: profile
                .paste_char_delay_ms
                .unwrap_or(10)
                .max(0) as u32,
            hex_view: profile.hex_view,
            timestamps: profile.timestamps,
        };
        self.terminal.update(cx, |t, _| {
            t.set_serial_tx(write_tx);
            t.set_profile_settings(settings);
        });

        // Optional session log. Opened lazily — failure to open
        // (no support dir, perm issue) is logged and the session
        // proceeds without recording, rather than refusing to
        // connect. The file is moved into the drain task and held
        // there for the session's lifetime; dropping the task on
        // disconnect drops the file, which closes it.
        let log_file = if profile.log_enabled {
            match open_session_log(&profile) {
                Ok((file, path)) => {
                    log::info!("session log: {}", path.display());
                    Some(file)
                }
                Err(e) => {
                    log::error!("session log: open failed: {e}");
                    None
                }
            }
        } else {
            None
        };

        // Spawn the read drain. Held in `drain_task` so a
        // subsequent connect cancels this one by dropping the
        // task field. The log file is moved into the closure so
        // it lives as long as the drain task. On natural loop
        // exit (read_rx closed because the OS read thread saw
        // the device disappear, etc.) the task notifies AppView
        // so it can decide whether to auto-reconnect — a
        // user-initiated disconnect drops this Task entirely
        // so the post-loop notification never runs.
        let weak_terminal = self.terminal.downgrade();
        let weak_app = cx.entity().downgrade();
        let read_rx = channels.read_rx;
        let task = cx.spawn(async move |_, cx| {
            let mut log = log_file;
            while let Ok(bytes) = read_rx.recv_async().await {
                if let Some(f) = log.as_mut() {
                    if let Err(e) = f.write_all(&bytes) {
                        log::error!("session log write: {e}");
                        // Stop trying to write to a broken file —
                        // drop it so we don't keep erroring per
                        // chunk. Terminal output keeps flowing.
                        log = None;
                    }
                }
                if weak_terminal
                    .update(cx, |t, cx| t.feed_bytes(&bytes, cx))
                    .is_err()
                {
                    break;
                }
            }
            weak_app
                .update(cx, |app, cx| app.on_drain_ended(cx))
                .ok();
        });
        self.drain_task = Some(task);
        self.serial_disconnect = Some(channels.disconnect);
        self.connected_profile_id = Some(profile.id);
        // A successful (re)connect ends any auto-reconnect window —
        // the right pane should now render off the live session, not
        // the placeholder stand-in that kept the terminal visible
        // while we were polling.
        self.auto_reconnect_for = None;
        // Clear any stale failure message from a prior attempt
        // (e.g. earlier ticks of the auto-reconnect retry loop).
        self.connect_error = None;
    }

    /// Open the form for a new profile, seeded from `Profile::defaults`.
    /// Idempotent if already open — re-creates the field state so the
    /// user gets a fresh form rather than whatever they typed before.
    fn open_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.editor = Some(build_editor(None, &Profile::defaults(), window, cx));
        cx.notify();
    }

    /// Open the form for an existing profile. Pre-fills every field
    /// (text inputs + selects + checkbox) with the profile's current
    /// values. Silently no-ops if the id has vanished from the store
    /// between row-render and click (rare; the store is the single
    /// source of truth and the sidebar re-reads it every render).
    fn open_editor_for(
        &mut self,
        id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(profile) = self.profile_store.get(&id) else {
            return;
        };
        self.editor = Some(build_editor(Some(id), &profile, window, cx));
        cx.notify();
    }

    fn cancel_editor(&mut self, cx: &mut Context<Self>) {
        self.editor = None;
        cx.notify();
    }

    /// Called by the drain task when the read channel closes
    /// without a matching user-initiated teardown (the user
    /// teardown drops the drain Task entirely, which cancels the
    /// future before this notification ever fires). Treats the
    /// drop as a session loss: clears the connected state, and if
    /// the active profile has `auto_reconnect` set, starts the
    /// retry-poll task. Otherwise just logs and lets the right
    /// pane fall back to the welcome screen.
    fn on_drain_ended(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.connected_profile_id.take() else {
            // User teardown already cleared this — nothing to do.
            return;
        };
        // The OS threads may still be wrapping up; tell them to
        // stop and detach so the file descriptor is released
        // promptly. New connect attempts handle the small race
        // with their open-retry loop.
        if let Some(d) = self.serial_disconnect.take() {
            d.shutdown();
        }
        self.drain_task = None;
        self.terminal.update(cx, |t, _| t.clear_serial_tx());

        let Some(profile) = self.profile_store.get(&id) else {
            log::warn!("session dropped (profile {id} no longer exists)");
            cx.notify();
            return;
        };
        if !profile.auto_reconnect {
            log::warn!("session dropped (auto-reconnect off)");
            cx.notify();
            return;
        }
        log::warn!("session dropped — auto-reconnecting");
        // Hold the profile id in `auto_reconnect_for` so the right
        // pane keeps the terminal viewport on screen during the
        // retry window. The retry-task's eventual `connect_to`
        // call will clear this and set `connected_profile_id` on
        // success; the timeout branch clears it then.
        self.auto_reconnect_for = Some(id);
        self.start_auto_reconnect(profile, cx);
        // Trigger a repaint so the session header swaps the green
        // dot for amber and the status bar shows "Reconnecting…"
        // immediately, instead of waiting for the next retry tick.
        cx.notify();
    }

    /// Spawn a polling task that retries `connect_to` until the
    /// port reappears (or the retry budget runs out). Replaces
    /// any prior reconnect task so user actions that cancel via
    /// `auto_reconnect_task = None` win cleanly.
    fn start_auto_reconnect(&mut self, profile: Profile, cx: &mut Context<Self>) {
        let weak = cx.entity().downgrade();
        let task = cx.spawn(async move |_, cx| {
            // ~30s budget at 2s per attempt — matches the Tauri
            // form's "poll for the port to reappear (up to 30s)"
            // hint shown next to the auto-reconnect checkbox.
            for _ in 0..15 {
                cx.background_executor()
                    .timer(AUTO_RECONNECT_INTERVAL)
                    .await;
                let succeeded = weak
                    .update(cx, |app, cx| {
                        app.connect_to(profile.clone(), cx);
                        app.connected_profile_id.is_some()
                    })
                    .ok()
                    .unwrap_or(false);
                if succeeded {
                    return;
                }
            }
            log::warn!(
                "auto-reconnect to {} gave up after {} attempts",
                profile.port_name,
                15
            );
            weak.update(cx, |app, cx| {
                app.auto_reconnect_task = None;
                // Drop the visual stand-in so the right pane falls
                // back to the welcome screen now that we've stopped
                // trying.
                app.auto_reconnect_for = None;
                cx.notify();
            })
            .ok();
        });
        self.auto_reconnect_task = Some(task);
    }

    /// Tear down the active serial session and reopen the editor
    /// for the same profile, mirroring the Tauri version's flow
    /// (terminal → disconnect → back to profile settings). Same
    /// shutdown order as the start of `connect_to`: drop the
    /// channel ends first, then `wait` on the thread join so the
    /// port file descriptor is fully released before any future
    /// reopen.
    fn disconnect_current(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.connected_profile_id.take() else {
            return;
        };
        self.drain_task = None;
        // Cancel any pending auto-reconnect — user explicitly
        // disconnecting overrides whatever retry loop might be
        // mid-flight.
        self.auto_reconnect_task = None;
        self.auto_reconnect_for = None;
        self.terminal.update(cx, |t, _| t.clear_serial_tx());
        // Flag the OS threads to exit (non-blocking). The threads
        // wind down within ~SHUTDOWN_POLL_INTERVAL and release the
        // port file descriptor so the next `connect_to` won't fail
        // with "Unable to acquire exclusive lock". Synchronous
        // `join` would freeze the UI when a thread is stuck on
        // `port.write_all` or a disconnect ioctl.
        if let Some(d) = self.serial_disconnect.take() {
            d.shutdown();
        }
        self.open_editor_for(id, window, cx);
    }

    /// Switch the active sub-tab on the open editor. No-op if the
    /// editor isn't open or the requested tab is already active —
    /// avoids an unnecessary re-render.
    fn set_editor_tab(&mut self, tab: EditorTab, cx: &mut Context<Self>) {
        let Some(ed) = self.editor.as_mut() else {
            return;
        };
        if ed.tab == tab {
            return;
        }
        ed.tab = tab;
        cx.notify();
    }

    /// Rebuild the Serial Port select's option list from the current
    /// OS port enumeration. Preserves the current selection across
    /// the rescan (so plugging a new device in doesn't deselect the
    /// one the user already picked) and falls back to a "(not
    /// connected)" entry if the previously-selected port is no
    /// longer detected.
    fn rescan_ports(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(ed) = self.editor.as_ref() else {
            return;
        };
        let current = read_select(&ed.port, cx);
        let opts = port_opts(&current);
        let port_state = ed.port.clone();
        port_state.update(cx, |state, cx| {
            state.set_items(opts, window, cx);
            state.set_selected_value(&current, window, cx);
        });
        cx.notify();
    }

    /// Pull text out of the form, build a `Profile`, persist via the
    /// store. In create mode this fills the rest of the fields from
    /// `Profile::defaults()` (8N1 / no flow control / CR); in edit
    /// mode it re-fetches the existing profile and only overwrites
    /// the three editable fields, preserving `theme_id`, `highlight`,
    /// `auto_reconnect`, etc. that the prototype's form doesn't
    /// expose. On success the form closes; on validation failure the
    /// inline error is set and the form stays open so the user can
    /// fix it.
    /// Persist the form to the store. Returns the saved profile's id
    /// on success so callers (e.g. the Connect button) can chain a
    /// connect off the back of a save without redundant lookups.
    fn save_editor(&mut self, cx: &mut Context<Self>) -> Option<String> {
        let editor = self.editor.as_ref()?;
        let mut base = match editor.profile_id.as_ref() {
            // Edit: start from the existing profile so untouched
            // fields (theme, paste settings, timestamps, …) survive
            // a partial-form save.
            Some(id) => match self.profile_store.get(id) {
                Some(p) => p,
                None => {
                    if let Some(ed) = self.editor.as_mut() {
                        ed.error = Some("profile no longer exists".into());
                    }
                    cx.notify();
                    return None;
                }
            },
            // Create: defaults supply everything the form doesn't touch.
            None => Profile::defaults(),
        };
        apply_editor_to_profile(editor, &mut base, cx);

        let result = match editor.profile_id.as_ref() {
            None => self.profile_store.create(base.clone()).map(|p| p.id),
            Some(_) => self.profile_store.update(base.clone()).map(|p| p.id),
        };

        match result {
            Ok(id) => {
                // Keep the editor open after Save so the user can
                // tweak more settings (or click Connect) without
                // losing the form. Refresh:
                //   * `profile_id` for the create-then-edit case so
                //     a follow-up Save updates instead of duping;
                //   * `baseline` so the dirty-detection in render
                //     resets to "clean" (Save button dims).
                let fresh_baseline = self.profile_store.get(&id);
                if let Some(ed) = self.editor.as_mut() {
                    ed.profile_id = Some(id.clone());
                    ed.error = None;
                    if let Some(b) = fresh_baseline {
                        ed.baseline = b;
                    }
                }
                cx.notify();
                Some(id)
            }
            Err(e) => {
                if let Some(ed) = self.editor.as_mut() {
                    ed.error = Some(format!("{e}"));
                }
                cx.notify();
                None
            }
        }
    }

    /// Save the form, then immediately connect to the resulting
    /// profile. Mirrors the Tauri form's "Connect" button: the
    /// primary action turns the editor flow into a single click for
    /// the common case of "make this profile and use it now." Note
    /// it bypasses `select_profile` (which under the new flow opens
    /// the editor) and calls `connect_to` directly — saving has
    /// already closed the editor, and we want the next step to be
    /// the terminal, not back into the editor we just left.
    fn save_and_connect(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.save_editor(cx) else {
            return;
        };
        let Some(profile) = self.profile_store.get(&id) else {
            self.connect_error = Some("profile not found".into());
            cx.notify();
            return;
        };
        // Save keeps the editor open (so the user can keep tuning),
        // but Connect explicitly transitions to the terminal — close
        // the editor here so the right pane renders the live session
        // instead of the form.
        self.editor = None;
        self.selected_profile_id = Some(id);
        self.connect_error = None;
        // User-initiated connect supersedes any in-flight auto-
        // reconnect retry from a previous session — drop the Task
        // here (connect_to itself can't, since the retry loop calls
        // connect_to and we'd be dropping our own running future).
        self.auto_reconnect_task = None;
        self.auto_reconnect_for = None;
        self.connect_to(profile, cx);
        // Pull focus into the terminal so the very next keystroke
        // (Enter, anything) reaches the on_key_down handler that
        // forwards bytes to the serial port. Without this the
        // editor's last-focused input keeps focus, the terminal
        // appears, but typing goes nowhere.
        let viewport_focus = self.terminal.read(cx).focus_handle().clone();
        viewport_focus.focus(window, cx);
    }

    /// Delete the profile currently being edited. If we're connected
    /// to it, tear that connection down first so we don't keep an
    /// open serial port pointing at a deleted profile id. Selection
    /// state for the deleted id is also cleared so the sidebar
    /// doesn't try to highlight a missing row on the next render.
    fn delete_from_editor(&mut self, cx: &mut Context<Self>) {
        let Some(editor) = self.editor.as_ref() else {
            return;
        };
        let Some(id) = editor.profile_id.clone() else {
            return;
        };
        match self.profile_store.delete(&id) {
            Ok(()) => {
                if self.connected_profile_id.as_deref() == Some(id.as_str())
                    || self.auto_reconnect_for.as_deref() == Some(id.as_str())
                {
                    self.drain_task = None;
                    self.connected_profile_id = None;
                    self.auto_reconnect_task = None;
                    self.auto_reconnect_for = None;
                    self.terminal.update(cx, |t, _| t.clear_serial_tx());
                    if let Some(d) = self.serial_disconnect.take() {
                        d.shutdown();
                    }
                }
                if self.selected_profile_id.as_deref() == Some(id.as_str()) {
                    self.selected_profile_id = None;
                    self.connect_error = None;
                }
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

    /// Open (or focus) the standalone Settings window. Mirrors the
    /// Tauri shape — a separate OS window so the user can change
    /// theme/skin and watch the main window update live without
    /// flipping past a modal layer. If a Settings window is already
    /// open we just bring it to the front; otherwise we spawn a
    /// fresh one with `SettingsView` as the root.
    fn open_settings(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        // Reuse the existing window if it's still alive. `update`
        // returns `Err` when the OS window has been closed; that's
        // the signal to drop the stale handle and open a new one.
        if let Some(handle) = &self.settings_window {
            let activated = handle
                .update(cx, |_, window, _| window.activate_window())
                .is_ok();
            if activated {
                return;
            }
            self.settings_window = None;
        }

        let bus = self.settings_bus.clone();
        let skins = self.skins_store.clone();
        let highlight = self.highlight_store.clone();
        let themes = self.themes_store.clone();
        let bounds = Bounds::centered(None, gpui::size(px(720.0), px(640.0)), cx);
        let opened = cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("Settings · Baudrun".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            move |window, cx| {
                let view = cx.new(|cx| {
                    SettingsView::new(bus, skins, highlight, themes, window, cx)
                });
                cx.new(|cx| Root::new(view, window, cx))
            },
        );
        match opened {
            Ok(handle) => {
                self.settings_window = Some(handle);
            }
            Err(err) => {
                log::error!("open settings window: {err}");
            }
        }
    }
}

impl Render for AppView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let profiles = self.profile_store.list();
        let selected = self.selected_profile_id.clone();
        let connected = self.connected_profile_id.clone();
        // gpui-component's dialog / notification / sheet overlays
        // don't render unless we explicitly ask for them here. The
        // gpui_component::Root wrap in main.rs only paints `view`
        // and the tooltip layer; everything else has to be added
        // by the user (story/lib.rs follows the same pattern).
        // Without this, `window.open_alert_dialog` queues the
        // dialog into Root's `active_dialogs` Vec but nothing
        // ever paints it — the build closure only fires from
        // `render_dialog_layer`.
        let dialog_layer = Root::render_dialog_layer(window, cx);
        let notification_layer = Root::render_notification_layer(window, cx);

        // Right pane: three branches.
        //   * editor is open  → form
        //   * connected to a profile → terminal viewport + session header
        //   * otherwise → welcome screen (matches the Tauri version's
        //     idle state: a centered "pick a profile" splash, no
        //     terminal viewport visible)
        // Branching here (vs. conditionally adding children to a
        // shared div) lets each branch pick its own padding /
        // background without leaking into the other.
        // Show the terminal pane both for live sessions (real serial
        // bytes flowing) and for the auto-reconnect retry window
        // (port dropped, retry-poll task running). Without the
        // second arm the right pane flickers back to the welcome
        // screen between every retry tick — even though the user
        // hasn't asked to disconnect.
        let connected_profile = self
            .connected_profile_id
            .as_ref()
            .or(self.auto_reconnect_for.as_ref())
            .and_then(|id| self.profile_store.get(id));
        // True when we're showing the terminal off the reconnect
        // stand-in rather than a live session — drives the amber
        // dot and the "Reconnecting…" labels.
        let reconnecting =
            self.connected_profile_id.is_none() && self.auto_reconnect_for.is_some();
        let has_profiles = !self.profile_store.list().is_empty();
        let right_pane: gpui::AnyElement = match self.editor.as_ref() {
            Some(editor) => form_pane(EditorRender::from(editor, cx), cx).into_any_element(),
            None => match connected_profile.clone() {
                Some(profile) => {
                    let terminal = self.terminal.clone();
                    div()
                        .flex_1()
                        // Without `min_w_0`, the terminal viewport's
                        // intrinsic min-width (cols × cell_w) is
                        // taken as the pane's min-width and the
                        // pane grows past the window edge — pushing
                        // anything to the right of the header
                        // (Clear / Disconnect) off-screen. Setting
                        // `min_w_0` lets the pane shrink to the
                        // window width and the grid clips any
                        // overflow instead.
                        .min_w_0()
                        .h_full()
                        .flex()
                        .flex_col()
                        .child(session_header(profile, reconnecting, cx))
                        .child(div().flex_1().min_h_0().child(terminal))
                        .into_any_element()
                }
                None => welcome_pane(has_profiles).into_any_element(),
            },
        };

        // Editor's profile name (for the status bar) — read at
        // render time so a fresh edit session doesn't show a
        // stale value. `None` for the editor key means "no editor
        // open" — the status bar uses its other arms in that case.
        let editor_name: Option<String> = self.editor.as_ref().map(|e| {
            e.name.read(cx).value().to_string()
        });
        // Connected profile lookup is reused from above (right_pane
        // computed it for the session header). Cloned for the
        // status bar so both renders can use it.
        let connected_for_status = connected_profile.clone();

        div()
            .size_full()
            .relative()
            .child(
                div()
                    .size_full()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .flex_1()
                            .min_h_0()
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
                    // Inherits the theme's font_family, which the
                    // gpui-component Root sets to `.SystemUIFont`
                    // (SF Pro on macOS, Segoe on Windows). Don't
                    // override with Menlo here — that's mono, and
                    // chrome should look like chrome. Terminal pane
                    // sets its own font internally so it stays
                    // monospaced.
                    .child(sidebar_header(cx))
                    .children(profiles.into_iter().map(|profile| {
                        let is_selected = selected.as_deref() == Some(profile.id.as_str());
                        let is_connected = connected.as_deref() == Some(profile.id.as_str());
                        // Treat the row as still "in-session" while
                        // we're auto-reconnecting to it. Otherwise
                        // the per-tick `open … No such file` error
                        // from connect_to flashes a red dot + the
                        // inline error under the row, even though
                        // the header is already communicating the
                        // retry — the duplicate signal is noisy and
                        // implies the session is broken when it's
                        // actually mid-recovery. Tauri does the
                        // same: sidebar stays green, only the
                        // session header swaps to the amber pulse.
                        let is_reconnecting = self
                            .auto_reconnect_for
                            .as_deref()
                            == Some(profile.id.as_str());
                        let row_error = if is_selected && !is_reconnecting {
                            self.connect_error.clone()
                        } else {
                            None
                        };
                        // Connected wins over Failed when both apply
                        // (shouldn't happen — connect_to clears the
                        // error before setting connected — but
                        // defining the precedence keeps the
                        // indicator stable if that invariant ever
                        // drifts). Reconnecting takes its own slot
                        // so the sidebar dot can pulse amber in
                        // lockstep with the session header.
                        let status = if is_connected {
                            Some(RowStatus::Connected)
                        } else if is_reconnecting {
                            Some(RowStatus::Reconnecting)
                        } else if is_selected && row_error.is_some() {
                            Some(RowStatus::Failed)
                        } else {
                            None
                        };
                        profile_row(profile, is_selected, status, row_error, cx)
                    })),
            )
                            // -- right pane: form OR terminal --
                            .child(right_pane),
                    )
                    // -- bottom status bar (sits under both panes) --
                    .child(status_bar(
                        connected_for_status.as_ref(),
                        reconnecting,
                        editor_name.as_deref(),
                    )),
            )
            // -- gpui-component overlay layers (dialogs, toasts) --
            .children(dialog_layer)
            .children(notification_layer)
    }
}

/// Idle splash screen — shown when the app is launched with no
/// connected profile and the user hasn't opened the editor yet.
/// Mirrors the Tauri version's "no terminal until you pick a
/// profile" default. Wording adapts to whether any profiles
/// exist: with profiles, prompt to pick one; without, prompt to
/// click the `+` to create one.
fn welcome_pane(has_profiles: bool) -> impl IntoElement {
    let prompt = if has_profiles {
        "Pick a profile from the sidebar to start a session."
    } else {
        "Click the + above the profile list to create one."
    };
    div()
        .flex_1()
        .h_full()
        .bg(rgb(FORM_BG))
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap_3()
        .child(
            div()
                .text_size(px(28.0))
                .text_color(rgb(SIDEBAR_FG))
                .child("Baudrun"),
        )
        .child(
            div()
                .text_size(px(13.0))
                .text_color(rgba(FG_SECONDARY))
                .child(prompt),
        )
}

/// Session header above the terminal viewport. Shows status dot +
/// profile name + connection meta on the left, and Clear /
/// Disconnect buttons on the right. Only rendered when a profile
/// is actually connected — loopback / no-device modes hide the
/// header so the prototype's no-profile path stays minimal.
fn session_header(
    profile: Profile,
    reconnecting: bool,
    cx: &mut Context<AppView>,
) -> impl IntoElement {
    let parity_letter = match profile.parity.as_str() {
        "odd" => "O",
        "even" => "E",
        "mark" => "M",
        "space" => "S",
        _ => "N",
    };
    // Mirror the Tauri header: full session line always, with
    // " · reconnecting…" appended during a retry window. The
    // appended phrase keeps the port/baud/8N1 info visible so the
    // user knows what the retry is targeting.
    let mut meta = format!(
        "{} · {} /{} {} {}",
        profile.port_name,
        profile.baud_rate,
        profile.data_bits,
        parity_letter,
        profile.stop_bits,
    );
    if reconnecting {
        meta.push_str(" · reconnecting…");
    }
    let dot_color = if reconnecting {
        STATUS_RECONNECTING
    } else {
        STATUS_CONNECTED
    };
    div()
        .w_full()
        .px_4()
        .py_2()
        .bg(rgb(FORM_BG))
        .border_b_1()
        .border_color(rgba(BORDER_SUBTLE))
        .text_size(px(13.0))
        .text_color(rgb(SIDEBAR_FG))
        .flex()
        .flex_row()
        .items_center()
        .gap_3()
        .child(
            div()
                .flex_1()
                .flex()
                .flex_row()
                .items_center()
                .gap_2()
                .child({
                    let dot = div()
                        .w(px(STATUS_DOT_PX))
                        .h(px(STATUS_DOT_PX))
                        .rounded_full()
                        .bg(rgb(dot_color));
                    if reconnecting {
                        // Match the Tauri `.dot.reconnecting`
                        // pulse: 1s ease-in-out, opacity bounces
                        // between roughly 0.35 and 1.0. gpui's
                        // `pulsating_between` returns the easing
                        // curve; the per-frame closure applies the
                        // current alpha.
                        dot.with_animation(
                            "session-header-reconnect-pulse",
                            Animation::new(Duration::from_secs(1))
                                .repeat()
                                .with_easing(pulsating_between(0.35, 1.0)),
                            |el, delta| el.opacity(delta),
                        )
                        .into_any_element()
                    } else {
                        dot.into_any_element()
                    }
                })
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .text_size(px(13.0))
                                .text_color(rgb(SIDEBAR_FG))
                                .child(profile.name),
                        )
                        .child(
                            div()
                                .text_size(px(11.0))
                                .text_color(rgba(FG_SECONDARY))
                                .child(meta),
                        ),
                ),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .gap_2()
                .child(pill_button("Clear", false).on_mouse_up(
                    MouseButton::Left,
                    cx.listener(|this, _: &MouseUpEvent, _window, cx| {
                        this.terminal.update(cx, |t, cx| t.clear_screen(cx));
                    }),
                ))
                .child(primary_button("Disconnect").on_mouse_up(
                    MouseButton::Left,
                    cx.listener(|this, _: &MouseUpEvent, window, cx| {
                        this.disconnect_current(window, cx);
                    }),
                )),
        )
}

/// Bottom-of-window status bar — single-line muted text, full
/// window width (sits under both the sidebar and the right pane).
/// Mirrors the Tauri version's footer status: shows the live
/// connection target when connected, the profile being edited
/// when the editor is open, and a neutral "Not connected" when
/// idle. Future slices will hang scan indicators, update toasts,
/// and the undo-delete countdown off this same row.
fn status_bar(
    connected: Option<&Profile>,
    reconnecting: bool,
    editing_profile_name: Option<&str>,
) -> impl IntoElement {
    let text = match (connected, editing_profile_name) {
        (Some(p), _) if reconnecting => {
            format!("Reconnecting to {} @ {}…", p.port_name, p.baud_rate)
        }
        (Some(p), _) => format!("Connected to {} @ {}", p.port_name, p.baud_rate),
        (None, Some(name)) if !name.is_empty() => format!("Editing {name}"),
        (None, Some(_)) => "Editing new profile".to_string(),
        (None, None) => "Not connected".to_string(),
    };
    div()
        .w_full()
        .px_4()
        .py_1()
        .bg(rgb(SIDEBAR_BG))
        .border_t_1()
        .border_color(rgba(BORDER_SUBTLE))
        .text_size(px(11.0))
        .text_color(rgba(FG_SECONDARY))
        .child(text)
}

/// Sidebar header row: muted "PROFILES" label on the left, "+"
/// affordance on the right that opens the new-profile form. The
/// "+" is a div-with-click rather than a real button widget — same
/// reasoning as the rest of the sidebar (less surface area than
/// adopting `gpui_component::button` for one element).
fn sidebar_header(cx: &mut Context<AppView>) -> impl IntoElement {
    // Shared chrome for the inline icon-buttons (+ and ⚙). Same
    // padding / hover treatment so they read as a pair.
    let icon_btn = || {
        div()
            .px_2()
            .rounded_sm()
            .text_color(rgb(SIDEBAR_FG))
            .hover(|s| s.bg(rgb(SIDEBAR_SELECTED)))
            .cursor_pointer()
    };
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
                .flex()
                .flex_row()
                .items_center()
                .gap_1()
                .child(
                    icon_btn()
                        .text_size(px(16.0))
                        .child("+")
                        .on_mouse_up(
                            MouseButton::Left,
                            cx.listener(|this, _: &MouseUpEvent, window, cx| {
                                this.open_editor(window, cx);
                            }),
                        ),
                )
                // Unicode gear (`⚙`). Avoids pulling in an icon
                // crate for a single chrome glyph; we can swap to
                // gpui-component's `Icon` later if more accents
                // arrive. Sized one px smaller than the `+` so the
                // two glyphs visually balance — `+` is a thin stroke,
                // the gear is a denser shape.
                .child(
                    icon_btn()
                        .text_size(px(15.0))
                        .child("\u{2699}")
                        .on_mouse_up(
                            MouseButton::Left,
                            cx.listener(|this, _: &MouseUpEvent, window, cx| {
                                this.open_settings(window, cx);
                            }),
                        ),
                ),
        )
}


// --- Baudrun (default) skin ---------------------------------------
//
// Colour vars are taken from `src-tauri/resources/builtin_skins.json`
// (the `baudrun` entry's dark `vars`). gpui's `rgba(0xRRGGBBAA)` is
// the inverse of CSS's `rgba(r, g, b, a/255)`, so each constant
// below is the hex form of the same value the Tauri build ships.
// Names mirror the CSS var names so it's a one-line lookup either
// direction.

/// Form-pane background — `--bg-main` (rgba(20,20,22,0.55))
/// composited over `--shell-bg` (#1d1d1e) and baked opaque.
/// Sits noticeably darker than the sidebar so the split between
/// the two reads visually without needing a heavy divider, and
/// makes the section cards (which are translucent white over this)
/// stand out as raised content the way they do in the Tauri
/// build.
const FORM_BG: u32 = 0x18181a;

/// `--fg-secondary` (65% white). Used for field labels (uppercase
/// "SERIAL PORT" etc.) and the header subtitle.
const FG_SECONDARY: u32 = 0xFFFFFFA6;
/// `--fg-tertiary` (40% white). Quieter still — the uppercase
/// mode-tag under the title ("EDIT PROFILE").
const FG_TERTIARY: u32 = 0xFFFFFF66;
/// `--border-subtle` (8% white). Section-card outlines + the
/// header bottom rule.
const BORDER_SUBTLE: u32 = 0xFFFFFF14;
/// `--bg-panel` (6% white). Section-card surface — sits a notch
/// above the form bg so the cards read as raised content.
const PANEL_BG: u32 = 0xFFFFFF0F;
/// `--accent`. Solid macOS-y blue used for the primary action
/// (Connect). Opaque (alpha 0xFF baked in).
const ACCENT: u32 = 0x0a84ffFF;
/// White text on the accent button — full-strength, not the muted
/// `--fg-primary`, because the blue background needs more contrast.
const ACCENT_FG: u32 = 0xFFFFFFFF;
/// Active-tab background — `--bg-active` from the Baudrun skin
/// (rgba(0,122,255,0.25)). Tints the left-rail entry for the
/// selected sub-tab without going so loud the entry looks like a
/// primary button.
const TAB_ACTIVE_BG: u32 = 0x007AFF40;

/// Pill-button background — `--bg-input` from the Baudrun skin
/// (8% white over the shell). Reads as a slightly-raised neutral
/// surface; lets *colour* be reserved for semantically loaded
/// actions (Connect = accent, Delete = danger) instead of
/// decoration.
const BTN_BG: u32 = 0xFFFFFF14;
/// Pill-button hover background — `--bg-input-focus` (12% white).
const BTN_BG_HOVER: u32 = 0xFFFFFF1F;
/// Pill-button text colour — `--fg-primary` (95% white).
const BTN_FG: u32 = 0xFFFFFFF2;
/// Destructive-action text — `--danger` (`#ff453a`, opaque).
const BTN_FG_DANGER: u32 = 0xff453aFF;

/// Construct a fresh `Editor` whose every widget (text inputs +
/// selects + checkbox bool) is seeded from `profile`. Shared by
/// the new-profile path (`profile = Profile::defaults()`) and the
/// edit-profile path (`profile = store.get(id).unwrap()`) so the
/// initialisation logic for each field exists in exactly one place.
fn build_editor(
    profile_id: Option<String>,
    profile: &Profile,
    window: &mut Window,
    cx: &mut Context<AppView>,
) -> Editor {
    let name = {
        let val = profile.name.clone();
        cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("My switch")
                .default_value(val)
        })
    };
    let port = make_select(port_opts(&profile.port_name), &profile.port_name, window, cx);
    let paste_delay_val = profile.paste_char_delay_ms.unwrap_or(10).to_string();
    let paste_char_delay_ms = cx.new(|cx| {
        InputState::new(window, cx)
            .placeholder("10")
            .default_value(paste_delay_val)
    });
    // Empty `dtr/rts` strings on a freshly-loaded profile fall back
    // to "default" for the select — the store accepts both, but
    // showing "default" in the dropdown is the same intent and
    // avoids a blank-looking field.
    fn policy_or_default(s: &str) -> &str {
        if s.is_empty() {
            "default"
        } else {
            s
        }
    }
    Editor {
        profile_id,
        tab: EditorTab::Connection,
        name,
        port,
        baud: make_select(baud_opts(), &profile.baud_rate.to_string(), window, cx),
        data_bits: make_select(data_bits_opts(), &profile.data_bits.to_string(), window, cx),
        parity: make_select(parity_opts(), &profile.parity, window, cx),
        stop_bits: make_select(stop_bits_opts(), &profile.stop_bits, window, cx),
        flow_control: make_select(flow_control_opts(), &profile.flow_control, window, cx),
        line_ending: make_select(line_ending_opts(), &profile.line_ending, window, cx),
        backspace_key: make_select(backspace_opts(), &profile.backspace_key, window, cx),
        local_echo: profile.local_echo,
        dtr_on_connect: make_select(
            line_policy_opts(),
            policy_or_default(&profile.dtr_on_connect),
            window,
            cx,
        ),
        rts_on_connect: make_select(
            line_policy_opts(),
            policy_or_default(&profile.rts_on_connect),
            window,
            cx,
        ),
        dtr_on_disconnect: make_select(
            line_policy_opts(),
            policy_or_default(&profile.dtr_on_disconnect),
            window,
            cx,
        ),
        rts_on_disconnect: make_select(
            line_policy_opts(),
            policy_or_default(&profile.rts_on_disconnect),
            window,
            cx,
        ),
        hex_view: profile.hex_view,
        timestamps: profile.timestamps,
        log_enabled: profile.log_enabled,
        auto_reconnect: profile.auto_reconnect,
        paste_warn_multiline: profile.paste_warn_multiline,
        paste_slow: profile.paste_slow,
        paste_char_delay_ms,
        error: None,
        scroll_handle: ScrollHandle::new(),
        baseline: profile.clone(),
    }
}

/// Read every widget in `editor` and write the values onto `profile`,
/// in place. Fields the form doesn't expose (theme, paste settings,
/// auto-reconnect, …) are left untouched, which is what makes the
/// edit-path safe to round-trip.
fn apply_editor_to_profile(editor: &Editor, profile: &mut Profile, cx: &Context<AppView>) {
    profile.name = editor.name.read(cx).value().to_string();
    profile.port_name = read_select(&editor.port, cx);
    // Empty / non-numeric → 0, which `validate` rejects with
    // `InvalidBaud`; `Profile::data_bits` is i32 too. Let the store
    // be the single source of truth for what counts as valid rather
    // than duplicating its rules in the UI.
    profile.baud_rate = read_select(&editor.baud, cx).trim().parse().unwrap_or(0);
    profile.data_bits = read_select(&editor.data_bits, cx)
        .trim()
        .parse()
        .unwrap_or(0);
    profile.parity = read_select(&editor.parity, cx);
    profile.stop_bits = read_select(&editor.stop_bits, cx);
    profile.flow_control = read_select(&editor.flow_control, cx);
    profile.line_ending = read_select(&editor.line_ending, cx);
    profile.backspace_key = read_select(&editor.backspace_key, cx);
    profile.local_echo = editor.local_echo;
    profile.dtr_on_connect = read_select(&editor.dtr_on_connect, cx);
    profile.rts_on_connect = read_select(&editor.rts_on_connect, cx);
    profile.dtr_on_disconnect = read_select(&editor.dtr_on_disconnect, cx);
    profile.rts_on_disconnect = read_select(&editor.rts_on_disconnect, cx);
    profile.hex_view = editor.hex_view;
    profile.timestamps = editor.timestamps;
    profile.log_enabled = editor.log_enabled;
    profile.auto_reconnect = editor.auto_reconnect;
    profile.paste_warn_multiline = editor.paste_warn_multiline;
    profile.paste_slow = editor.paste_slow;
    // Empty / non-numeric → None (rolls back to the store's default
    // of 10ms via `Profile::defaults` on next load). Negative values
    // collapse to 0, which the store accepts.
    let delay_str = editor.paste_char_delay_ms.read(cx).value().to_string();
    profile.paste_char_delay_ms = delay_str.trim().parse::<i32>().ok().map(|v| v.max(0));
}

/// How long the auto-reconnect poll waits between
/// `serial_io::open` retries. 2s × 15 attempts = ~30s budget,
/// matching the Tauri profile form's "poll for the port to
/// reappear (up to 30s) and reopen transparently" hint shown
/// next to the auto-reconnect checkbox.
const AUTO_RECONNECT_INTERVAL: std::time::Duration = std::time::Duration::from_secs(2);

/// Open a session log file for the given profile. Path is
/// `<support_dir>/logs/<slug>_<YYYY-MM-DD_HHMMSS>.log` — same
/// shape the Tauri version uses, so log dirs port cleanly when
/// the migration completes. Returns `(writer, path)`; the writer
/// is a `SanitizingLogWriter` so the on-disk file reads like the
/// terminal view (Cisco-style `\r\r\n` collapses to `\n`, ANSI
/// CSI/OSC escapes are stripped, BS/CR-overwrite tricks are
/// applied) instead of the raw wire bytes.
fn open_session_log(
    profile: &Profile,
) -> std::io::Result<(SanitizingLogWriter<File>, std::path::PathBuf)> {
    let support = appdata::support_dir().map_err(std::io::Error::other)?;
    let dir = support.join("logs");
    std::fs::create_dir_all(&dir)?;
    let stamp = chrono::Local::now().format("%Y-%m-%d_%H%M%S");
    let filename = format!("{}_{stamp}.log", slugify_name(&profile.name));
    let path = dir.join(filename);
    let file = File::create(&path)?;
    Ok((SanitizingLogWriter::new(file), path))
}

/// Filename-safe slug for a profile name: lowercase ASCII + dash
/// for separators, anything else dropped, falls back to "session"
/// when the name reduces to nothing. Matches the Tauri version's
/// `slugify_session_name` in `src-tauri/src/commands/serial.rs`
/// so identically-named profiles produce identically-named log
/// files across the two builds.
fn slugify_name(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for ch in name.chars().flat_map(char::to_lowercase) {
        match ch {
            'a'..='z' | '0'..='9' => out.push(ch),
            ' ' | '-' | '_' | '.' => out.push('-'),
            _ => {}
        }
    }
    let trimmed = out.trim_matches('-').to_string();
    if trimmed.is_empty() {
        "session".into()
    } else {
        trimmed
    }
}

/// Compare two profiles on the fields the editor actually exposes
/// — drives the Save button's "dirty" state. Skipping `id` /
/// `created_at` / `updated_at` because those aren't user-editable
/// (id is set by the store on create, timestamps update on save)
/// and would otherwise spuriously flag every edited profile as
/// "dirty" right after save.
fn editor_fields_match(a: &Profile, b: &Profile) -> bool {
    a.name == b.name
        && a.port_name == b.port_name
        && a.baud_rate == b.baud_rate
        && a.data_bits == b.data_bits
        && a.parity == b.parity
        && a.stop_bits == b.stop_bits
        && a.flow_control == b.flow_control
        && a.line_ending == b.line_ending
        && a.backspace_key == b.backspace_key
        && a.local_echo == b.local_echo
        && a.dtr_on_connect == b.dtr_on_connect
        && a.rts_on_connect == b.rts_on_connect
        && a.dtr_on_disconnect == b.dtr_on_disconnect
        && a.rts_on_disconnect == b.rts_on_disconnect
        && a.hex_view == b.hex_view
        && a.timestamps == b.timestamps
        && a.log_enabled == b.log_enabled
        && a.auto_reconnect == b.auto_reconnect
        && a.paste_warn_multiline == b.paste_warn_multiline
        && a.paste_slow == b.paste_slow
        && a.paste_char_delay_ms == b.paste_char_delay_ms
}

/// One choice in a select widget. `id` is the canonical value
/// stored on the `Profile` (e.g. `"none"`, `"9600"`, `"crlf"`);
/// `title` is the human-readable label shown in the menu and as
/// the closed-state value (e.g. `"None"`, `"9600 (default)"`,
/// `"CRLF (\\r\\n) — modems"`). Cheap to clone (two `String`s) —
/// the option lists are tiny and built once per editor open.
#[derive(Clone)]
struct Opt {
    id: String,
    title: SharedString,
}

impl Opt {
    fn new(id: &str, title: &str) -> Self {
        Self {
            id: id.to_string(),
            title: SharedString::from(title.to_string()),
        }
    }
}

impl SelectItem for Opt {
    type Value = String;

    fn title(&self) -> SharedString {
        self.title.clone()
    }

    fn value(&self) -> &Self::Value {
        &self.id
    }
}

/// Build a `SelectState<Vec<Opt>>` pre-selected to whichever option
/// in `opts` has `id == selected`. If `selected` doesn't match
/// anything, no option is pre-selected (the closed-state shows the
/// placeholder, if any). Wraps the gpui-component constructor so
/// caller sites don't have to deal with `IndexPath` directly.
fn make_select(
    opts: Vec<Opt>,
    selected: &str,
    window: &mut Window,
    cx: &mut Context<AppView>,
) -> Entity<SelectState<Vec<Opt>>> {
    let idx = opts
        .iter()
        .position(|o| o.id == selected)
        .map(IndexPath::new);
    cx.new(|cx| SelectState::new(opts, idx, window, cx))
}

/// Read the currently-selected id from a `SelectState<Vec<Opt>>`.
/// Falls back to an empty string if nothing is selected — the
/// `Profile` validator rejects empty strings for these fields, so
/// the user gets a clear error rather than a silent bad save.
fn read_select(state: &Entity<SelectState<Vec<Opt>>>, cx: &Context<AppView>) -> String {
    state.read(cx).selected_value().cloned().unwrap_or_default()
}

// --- Option lists --------------------------------------------------
//
// Mirrors the Tauri ProfileForm.svelte option arrays. Labels are
// hand-written to match: short id + parenthetical hint where the
// raw id alone (e.g. "cr", "del") would be opaque to a user who
// hasn't shipped serial-console code before. Wrapping each in a
// fn keeps the borrowed-vec ergonomics simple — gpui-component
// takes the `Vec<Opt>` by value into the SelectState.

fn baud_opts() -> Vec<Opt> {
    [
        ("9600", "9600 (default)"),
        ("19200", "19200"),
        ("38400", "38400"),
        ("57600", "57600"),
        ("115200", "115200"),
        ("230400", "230400"),
        ("460800", "460800"),
        ("921600", "921600"),
    ]
    .into_iter()
    .map(|(id, title)| Opt::new(id, title))
    .collect()
}

fn data_bits_opts() -> Vec<Opt> {
    ["5", "6", "7", "8"]
        .into_iter()
        .map(|s| Opt::new(s, s))
        .collect()
}

fn parity_opts() -> Vec<Opt> {
    [
        ("none", "None"),
        ("odd", "Odd"),
        ("even", "Even"),
        ("mark", "Mark"),
        ("space", "Space"),
    ]
    .into_iter()
    .map(|(id, title)| Opt::new(id, title))
    .collect()
}

fn stop_bits_opts() -> Vec<Opt> {
    ["1", "1.5", "2"].into_iter().map(|s| Opt::new(s, s)).collect()
}

fn flow_control_opts() -> Vec<Opt> {
    [
        ("none", "None"),
        ("rtscts", "RTS/CTS"),
        ("xonxoff", "XON/XOFF"),
    ]
    .into_iter()
    .map(|(id, title)| Opt::new(id, title))
    .collect()
}

fn line_ending_opts() -> Vec<Opt> {
    [
        ("cr", "CR (\\r) — switches, routers"),
        ("lf", "LF (\\n) — Linux consoles"),
        ("crlf", "CRLF (\\r\\n) — legacy / Windows"),
    ]
    .into_iter()
    .map(|(id, title)| Opt::new(id, title))
    .collect()
}

/// Line-policy options for DTR/RTS on connect/disconnect — pulled
/// verbatim from `src/lib/api.ts` (LINE_POLICIES). Empty-string id
/// is allowed by `Profile::validate` but isn't useful in the UI;
/// "default" carries the same semantics ("leave as-is").
fn line_policy_opts() -> Vec<Opt> {
    [
        ("default", "Default (leave as-is)"),
        ("assert", "Assert (high)"),
        ("deassert", "Deassert (low)"),
    ]
    .into_iter()
    .map(|(id, title)| Opt::new(id, title))
    .collect()
}

fn backspace_opts() -> Vec<Opt> {
    [
        ("del", "DEL (0x7F) — VT100, xterm, modern"),
        ("bs", "BS (0x08) — older Cisco, Foundry"),
    ]
    .into_iter()
    .map(|(id, title)| Opt::new(id, title))
    .collect()
}

/// Build the Serial Port select options from the current OS port
/// list. Each detected port becomes one option; the title bundles
/// the device path with whatever the enumerator found
/// (`/dev/cu.usbserial-XYZ — FT232R USB UART · FTDI`) — same shape
/// the Tauri form uses, so the user gets enough info to identify
/// the right adapter without opening System Settings.
///
/// If `keep_selected` is non-empty and isn't in the detected list,
/// it's prepended as an "(not connected)" option so the saved
/// profile still shows its port even when the device is unplugged.
/// On port enumeration failure we still want a usable form, so we
/// fall back to the keep_selected (if any) and otherwise an empty
/// list — the user can rescan later.
fn port_opts(keep_selected: &str) -> Vec<Opt> {
    let detected = ports::list_ports().unwrap_or_default();
    let mut opts: Vec<Opt> = detected
        .iter()
        .map(|p| {
            let mut title = p.name.clone();
            if !p.product.is_empty() {
                title.push_str(" — ");
                title.push_str(&p.product);
            }
            if !p.chipset.is_empty() {
                title.push_str(" · ");
                title.push_str(&p.chipset);
            }
            Opt::new(&p.name, &title)
        })
        .collect();
    if !keep_selected.is_empty() && !detected.iter().any(|p| p.name == keep_selected) {
        // Prepend so the user's saved port shows up first when
        // it isn't currently detected (cable unplugged, etc.).
        let title = format!("{keep_selected} (not connected)");
        opts.insert(0, Opt::new(keep_selected, &title));
    }
    opts
}

/// Pill button styled per the Baudrun skin. Neutral translucent
/// fill by default; `danger=true` swaps the foreground to system
/// red for destructive actions like Delete. Returns a bare `Div`
/// so the call site can attach `.on_mouse_up` etc. — the helper
/// just owns the visual styling.
fn pill_button(label: &'static str, danger: bool) -> gpui::Div {
    let fg = if danger { rgba(BTN_FG_DANGER) } else { rgba(BTN_FG) };
    div()
        .px_3()
        .py_1()
        .bg(rgba(BTN_BG))
        .text_color(fg)
        .text_size(px(13.0))
        .rounded_md()
        .cursor_pointer()
        .hover(|s| s.bg(rgba(BTN_BG_HOVER)))
        .child(label)
}

/// Primary action button — solid `--accent` blue, white text. Used
/// for the form's Connect button (the call-to-action). Same shape
/// as `pill_button` so they line up flush in a button row.
fn primary_button(label: &'static str) -> gpui::Div {
    div()
        .px_3()
        .py_1()
        .bg(rgba(ACCENT))
        .text_color(rgba(ACCENT_FG))
        .text_size(px(13.0))
        .rounded_md()
        .cursor_pointer()
        .child(label)
}

/// All Editor fields a render needs, cloned out so the call site
/// can hand `cx: &mut Context<AppView>` to the form helpers without
/// keeping `&self.editor` borrowed at the same time. Cloning is
/// cheap — `Entity<T>` is `Arc`-shaped — and it's only done once
/// per render.
struct EditorRender {
    is_edit: bool,
    is_dirty: bool,
    tab: EditorTab,
    name: Entity<InputState>,
    port: Entity<SelectState<Vec<Opt>>>,
    baud: Entity<SelectState<Vec<Opt>>>,
    data_bits: Entity<SelectState<Vec<Opt>>>,
    parity: Entity<SelectState<Vec<Opt>>>,
    stop_bits: Entity<SelectState<Vec<Opt>>>,
    flow_control: Entity<SelectState<Vec<Opt>>>,
    line_ending: Entity<SelectState<Vec<Opt>>>,
    backspace_key: Entity<SelectState<Vec<Opt>>>,
    local_echo: bool,
    dtr_on_connect: Entity<SelectState<Vec<Opt>>>,
    rts_on_connect: Entity<SelectState<Vec<Opt>>>,
    dtr_on_disconnect: Entity<SelectState<Vec<Opt>>>,
    rts_on_disconnect: Entity<SelectState<Vec<Opt>>>,
    hex_view: bool,
    timestamps: bool,
    log_enabled: bool,
    auto_reconnect: bool,
    paste_warn_multiline: bool,
    paste_slow: bool,
    paste_char_delay_ms: Entity<InputState>,
    error: Option<String>,
    scroll_handle: ScrollHandle,
}

impl EditorRender {
    fn from(e: &Editor, cx: &Context<AppView>) -> Self {
        // Derive a hypothetical "what would Save persist right now"
        // Profile by applying the live widget values onto a clone of
        // the saved baseline; any field difference flags the form
        // as dirty (drives the Save button's brightness).
        let mut current = e.baseline.clone();
        apply_editor_to_profile(e, &mut current, cx);
        let is_dirty = !editor_fields_match(&current, &e.baseline);
        Self {
            is_edit: e.profile_id.is_some(),
            is_dirty,
            tab: e.tab,
            name: e.name.clone(),
            port: e.port.clone(),
            baud: e.baud.clone(),
            data_bits: e.data_bits.clone(),
            parity: e.parity.clone(),
            stop_bits: e.stop_bits.clone(),
            flow_control: e.flow_control.clone(),
            line_ending: e.line_ending.clone(),
            backspace_key: e.backspace_key.clone(),
            local_echo: e.local_echo,
            dtr_on_connect: e.dtr_on_connect.clone(),
            rts_on_connect: e.rts_on_connect.clone(),
            dtr_on_disconnect: e.dtr_on_disconnect.clone(),
            rts_on_disconnect: e.rts_on_disconnect.clone(),
            hex_view: e.hex_view,
            timestamps: e.timestamps,
            log_enabled: e.log_enabled,
            auto_reconnect: e.auto_reconnect,
            paste_warn_multiline: e.paste_warn_multiline,
            paste_slow: e.paste_slow,
            paste_char_delay_ms: e.paste_char_delay_ms.clone(),
            error: e.error.clone(),
            scroll_handle: e.scroll_handle.clone(),
        }
    }
}

fn form_pane(er: EditorRender, cx: &mut Context<AppView>) -> impl IntoElement {
    div()
        .flex_1()
        // `min_w_0` lets the pane shrink below its intrinsic
        // content min-width so long card descriptions wrap to
        // fit instead of pushing the Connect button off-screen.
        // `min_h_0` is the same idea for height: in the parent
        // flex_row, cross-axis stretch tries to fit the pane to
        // the row's height, but without min_h_0 the form's
        // intrinsic content min-height keeps it tall, the
        // scrollable body never sees an overflow, and the
        // Scrollbar widget has nothing to render.
        .min_w_0()
        .min_h_0()
        .bg(rgb(FORM_BG))
        .text_color(rgb(SIDEBAR_FG))
        .text_size(px(13.0))
        .flex()
        .flex_col()
        .child(form_header(er.is_edit, er.is_dirty, er.name.clone(), cx))
        .child(form_body(er, cx))
}

/// Header bar: editable profile name as the visible title (no
/// input chrome — `appearance(false)` strips the border/bg so it
/// reads as a heading rather than a form field), uppercase mode
/// tag underneath, action buttons on the right.
fn form_header(
    is_edit: bool,
    is_dirty: bool,
    name: Entity<InputState>,
    cx: &mut Context<AppView>,
) -> impl IntoElement {
    let subtitle = if is_edit { "EDIT PROFILE" } else { "NEW PROFILE" };
    // Save button text-color is the only thing that changes for the
    // dirty state — pill bg stays the same so the button doesn't
    // visually "appear" mid-edit. Tertiary fg (40% white) when
    // clean reads as "no-op available," primary fg (95% white)
    // when dirty reads as "click me to persist your changes."
    let save_fg = if is_dirty {
        rgba(BTN_FG)
    } else {
        rgba(FG_TERTIARY)
    };
    let delete_btn = is_edit.then(|| {
        pill_button("Delete", true).on_mouse_up(
            MouseButton::Left,
            cx.listener(|this, _: &MouseUpEvent, _window, cx| {
                this.delete_from_editor(cx);
            }),
        )
    });
    div()
        .w_full()
        .px_6()
        .py_3()
        .border_b_1()
        .border_color(rgba(BORDER_SUBTLE))
        .flex()
        .flex_row()
        .items_center()
        .gap_4()
        .child(
            div()
                .flex_1()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    Input::new(&name)
                        .appearance(false)
                        // Baudrun skin's `--font-size-h1` is 24px.
                        // `Size::Size(px)` renders text at
                        // `px * 0.875` per gpui-component's
                        // `input_text_size`, so pass ~27.5 to land
                        // at 24px.
                        .with_size(gpui_component::Size::Size(px(27.5))),
                )
                .child(
                    div()
                        // The Input applies an internal 8px
                        // horizontal padding regardless of
                        // `appearance(false)` (see gpui-component
                        // input.rs: `input_px` is applied before
                        // the `.when(appearance)` chrome). Match
                        // it here so the subtitle's first letter
                        // sits flush under the title's first
                        // letter rather than 8px to its left.
                        .pl(px(8.0))
                        .text_size(px(10.0))
                        .text_color(rgba(FG_TERTIARY))
                        .child(subtitle),
                ),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap_2()
                .children(delete_btn)
                .child(
                    pill_button("Save", false)
                        .text_color(save_fg)
                        .on_mouse_up(
                            MouseButton::Left,
                            cx.listener(|this, _: &MouseUpEvent, _window, cx| {
                                this.save_editor(cx);
                            }),
                        ),
                )
                .child(pill_button("Cancel", false).on_mouse_up(
                    MouseButton::Left,
                    cx.listener(|this, _: &MouseUpEvent, _window, cx| {
                        this.cancel_editor(cx);
                    }),
                ))
                .child(primary_button("Connect").on_mouse_up(
                    MouseButton::Left,
                    cx.listener(|this, _: &MouseUpEvent, window, cx| {
                        this.save_and_connect(window, cx);
                    }),
                )),
        )
}

/// Form body: a left rail of sub-tabs (Connection / Advanced) +
/// the active tab's content. Tab content is capped to a fixed
/// width so the cards keep form-shaped proportions on a wide
/// window. Mirrors the Tauri form's layout one-for-one.
fn form_body(er: EditorRender, cx: &mut Context<AppView>) -> impl IntoElement {
    let active = er.tab;
    let content: gpui::AnyElement = match er.tab {
        EditorTab::Connection => div()
            .flex()
            .flex_col()
            .gap_3()
            .child(connection_card(
                er.port,
                er.baud,
                er.data_bits,
                er.parity,
                er.stop_bits,
                er.flow_control,
                cx,
            ))
            .child(terminal_card(
                er.line_ending,
                er.backspace_key,
                er.local_echo,
                cx,
            ))
            .into_any_element(),
        EditorTab::Advanced => advanced_pane(
            er.dtr_on_connect,
            er.rts_on_connect,
            er.dtr_on_disconnect,
            er.rts_on_disconnect,
            er.hex_view,
            er.timestamps,
            er.log_enabled,
            er.auto_reconnect,
            er.paste_warn_multiline,
            er.paste_slow,
            er.paste_char_delay_ms,
            cx,
        )
        .into_any_element(),
    };

    div()
        .flex_1()
        .min_h_0()
        .flex()
        .flex_row()
        .child(form_tab_nav(active, cx))
        .child(
            // Bare `gpui::overflow_y_scroll` (no widget wrap) is
            // what actually scrolls — gpui-component's `Scrollable`
            // wrapper measures the scroll area incorrectly inside
            // our nested flex layout and ends up reporting "fits,
            // no scrollbar needed." So we wire the ScrollHandle
            // ourselves: scroll content tracks it via
            // `track_scroll`, and a sibling
            // `vertical_scrollbar(&handle)` paints the visible
            // bar. The parent div is `relative` so the scrollbar
            // (positioned absolutely internally) anchors to it.
            // Padding lives INSIDE the scrollable child, not on
            // the viewport — otherwise the bottom padding gets
            // eaten and the user can't scroll past the last card.
            div()
                .relative()
                .flex_1()
                .h_full()
                .min_w_0()
                .min_h_0()
                .child(
                    div()
                        .id("form-body-scroll")
                        .size_full()
                        .min_w_0()
                        .min_h_0()
                        .track_scroll(&er.scroll_handle)
                        .overflow_y_scroll()
                        .child(
                            div()
                                .w_full()
                                .min_w_0()
                                .px_6()
                                .py_4()
                                .flex()
                                .flex_col()
                                .gap_3()
                                .child(content)
                                .children(er.error.map(|err| {
                                    div()
                                        .px_3()
                                        .py_2()
                                        .text_size(px(12.0))
                                        .text_color(rgb(SIDEBAR_ERROR))
                                        .child(err)
                                })),
                        ),
                )
                .vertical_scrollbar(&er.scroll_handle),
        )
}

/// Left-rail sub-tab navigation. Each entry is a clickable row;
/// the active one paints `--bg-active` (translucent blue) so the
/// selected state reads instantly. Highlighting is omitted per
/// product direction — a separate phase later.
fn form_tab_nav(active: EditorTab, cx: &mut Context<AppView>) -> impl IntoElement {
    let item = |label: &'static str, tab: EditorTab| {
        let is_active = tab == active;
        let bg = if is_active {
            rgba(TAB_ACTIVE_BG)
        } else {
            rgba(0x00000000)
        };
        let fg = if is_active {
            rgba(BTN_FG)
        } else {
            rgba(FG_SECONDARY)
        };
        div()
            .w_full()
            .px_3()
            .py(px(6.0))
            .rounded_md()
            .bg(bg)
            .text_color(fg)
            .cursor_pointer()
            .hover(|s| s.bg(rgba(BTN_BG)))
            .child(label)
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(move |this, _: &MouseUpEvent, _window, cx| {
                    this.set_editor_tab(tab, cx);
                }),
            )
    };

    div()
        .w(px(160.0))
        .h_full()
        .px_3()
        .py_4()
        .border_r_1()
        .border_color(rgba(BORDER_SUBTLE))
        .flex()
        .flex_col()
        .gap_1()
        .text_size(px(13.0))
        .child(item("Connection", EditorTab::Connection))
        .child(item("Advanced", EditorTab::Advanced))
}

/// One section of the form — a translucent panel with a heading,
/// optional description, and a body. Section title size is
/// `--font-size-section` (15px); description is the muted
/// `--fg-secondary`. Panel uses `--radius-lg` (10px) and
/// `--bg-panel` / `--border-subtle`.
fn section_card(title: &'static str, body: impl IntoElement) -> gpui::Div {
    section_card_with_desc(title, None, body)
}

fn section_card_with_desc(
    title: &'static str,
    description: Option<&'static str>,
    body: impl IntoElement,
) -> gpui::Div {
    let mut header = div()
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .text_size(px(15.0))
                .text_color(rgb(SIDEBAR_FG))
                .child(title),
        );
    if let Some(desc) = description {
        header = header.child(
            div()
                .text_size(px(12.0))
                .text_color(rgba(FG_SECONDARY))
                // gpui's text default is `whitespace: nowrap` — a
                // long description like the Control Lines blurb
                // would otherwise render as a single line and run
                // off the right edge of the window.
                .whitespace_normal()
                .child(desc),
        );
    }
    div()
        .w_full()
        .bg(rgba(PANEL_BG))
        .border_1()
        .border_color(rgba(BORDER_SUBTLE))
        .rounded(px(10.0))
        .px_4()
        .py_3()
        .flex()
        .flex_col()
        .gap_3()
        .child(header)
        .child(body)
}

/// Per-field label + widget pair. Label uses Baudrun's
/// `--font-size-label` (11px), `--label-transform: uppercase`
/// (passed in already shouted by the caller), and `--fg-secondary`.
/// Label is `whitespace_nowrap` because gpui defaults to wrap, and
/// short fixed strings like "SLOW-PASTE DELAY (MS)" wrapping mid-
/// label inside a narrow container looks broken.
fn labeled(label: &'static str, widget: impl IntoElement) -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .text_size(px(11.0))
                .text_color(rgba(FG_SECONDARY))
                .whitespace_nowrap()
                .child(label),
        )
        .child(widget)
}

fn connection_card(
    port: Entity<SelectState<Vec<Opt>>>,
    baud: Entity<SelectState<Vec<Opt>>>,
    data_bits: Entity<SelectState<Vec<Opt>>>,
    parity: Entity<SelectState<Vec<Opt>>>,
    stop_bits: Entity<SelectState<Vec<Opt>>>,
    flow_control: Entity<SelectState<Vec<Opt>>>,
    cx: &mut Context<AppView>,
) -> gpui::Div {
    // Serial port row: select on the left (flex_1), rescan icon
    // on the right. Click rescans the OS port list and reapplies
    // the current selection.
    let port_row = div()
        .flex()
        .flex_row()
        .items_end()
        .gap_2()
        .child(
            div()
                .flex_1()
                .min_w_0()
                .child(labeled("SERIAL PORT", Select::new(&port).small())),
        )
        .child(
            div()
                .px_2()
                .py_1()
                .bg(rgba(BTN_BG))
                .text_color(rgba(BTN_FG))
                .rounded_md()
                .cursor_pointer()
                .hover(|s| s.bg(rgba(BTN_BG_HOVER)))
                .child("↻")
                .on_mouse_up(
                    MouseButton::Left,
                    cx.listener(|this, _: &MouseUpEvent, window, cx| {
                        this.rescan_ports(window, cx);
                    }),
                ),
        );
    let body = div()
        .flex()
        .flex_col()
        .gap_3()
        .child(port_row)
        // Two-column rows of selects.
        .child(
            div()
                .flex()
                .flex_row()
                .gap_3()
                .child(
                    div()
                        .flex_1()
                        .child(labeled("BAUD RATE", Select::new(&baud).small())),
                )
                .child(
                    div()
                        .flex_1()
                        .child(labeled("DATA BITS", Select::new(&data_bits).small())),
                ),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .gap_3()
                .child(
                    div()
                        .flex_1()
                        .child(labeled("PARITY", Select::new(&parity).small())),
                )
                .child(
                    div()
                        .flex_1()
                        .child(labeled("STOP BITS", Select::new(&stop_bits).small())),
                ),
        )
        .child(labeled("FLOW CONTROL", Select::new(&flow_control).small()));
    section_card("Connection", body)
}

fn terminal_card(
    line_ending: Entity<SelectState<Vec<Opt>>>,
    backspace_key: Entity<SelectState<Vec<Opt>>>,
    local_echo: bool,
    cx: &mut Context<AppView>,
) -> gpui::Div {
    let body = div()
        .flex()
        .flex_col()
        .gap_3()
        .child(
            div()
                .flex()
                .flex_row()
                .gap_3()
                .child(
                    div()
                        .flex_1()
                        .child(labeled(
                            "SEND LINE ENDING",
                            Select::new(&line_ending).small(),
                        )),
                )
                .child(
                    div()
                        .flex_1()
                        .child(labeled(
                            "BACKSPACE SENDS",
                            Select::new(&backspace_key).small(),
                        )),
                ),
        )
        .child(bool_field(
            "local-echo",
            "Local echo",
            local_echo,
            cx,
            |ed, v| ed.local_echo = v,
        ));
    section_card("Terminal", body)
}

/// Generic checkbox row that writes back to the open editor when
/// toggled. The closure parameter (`F`) is how we get a typed
/// `&mut Editor` lookup at the right field — passing the field
/// name as a string would force runtime dispatch for no benefit.
fn bool_field<F>(
    id: &'static str,
    label: &'static str,
    checked: bool,
    cx: &mut Context<AppView>,
    set: F,
) -> gpui::Div
where
    F: Fn(&mut Editor, bool) + 'static,
{
    bool_field_hinted(id, label, None, checked, cx, set)
}

/// Like `bool_field`, but also renders a small muted hint string
/// to the right of the checkbox. Mirrors the Tauri form's "Hex
/// view ┄ show incoming bytes as hex dump" pattern.
fn bool_field_hinted<F>(
    id: &'static str,
    label: &'static str,
    hint: Option<&'static str>,
    checked: bool,
    cx: &mut Context<AppView>,
    set: F,
) -> gpui::Div
where
    F: Fn(&mut Editor, bool) + 'static,
{
    let cb = Checkbox::new(id)
        .checked(checked)
        .label(label)
        .on_click(cx.listener(move |this, checked: &bool, _window, cx| {
            if let Some(ed) = this.editor.as_mut() {
                set(ed, *checked);
            }
            cx.notify();
        }));
    let mut row = div()
        .flex()
        .flex_row()
        .items_center()
        .gap_3()
        .child(cb);
    if let Some(h) = hint {
        row = row.child(
            div()
                .text_size(px(12.0))
                .text_color(rgba(FG_SECONDARY))
                .child(h),
        );
    }
    row
}

#[allow(clippy::too_many_arguments)]
fn advanced_pane(
    dtr_on_connect: Entity<SelectState<Vec<Opt>>>,
    rts_on_connect: Entity<SelectState<Vec<Opt>>>,
    dtr_on_disconnect: Entity<SelectState<Vec<Opt>>>,
    rts_on_disconnect: Entity<SelectState<Vec<Opt>>>,
    hex_view: bool,
    timestamps: bool,
    log_enabled: bool,
    auto_reconnect: bool,
    paste_warn_multiline: bool,
    paste_slow: bool,
    paste_char_delay_ms: Entity<InputState>,
    cx: &mut Context<AppView>,
) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_3()
        .child(
            // Top-of-tab heading + description, mirroring the Tauri
            // form. Lives outside the cards so it groups them
            // visually under one umbrella concern.
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_size(px(18.0))
                        .text_color(rgb(SIDEBAR_FG))
                        .child("Advanced"),
                )
                .child(
                    div()
                        .text_size(px(12.0))
                        .text_color(rgba(FG_SECONDARY))
                        .whitespace_normal()
                        .child("Control lines, hex view, timestamps, session logging."),
                ),
        )
        .child(control_lines_card(
            dtr_on_connect,
            rts_on_connect,
            dtr_on_disconnect,
            rts_on_disconnect,
        ))
        .child(output_card(
            hex_view,
            timestamps,
            log_enabled,
            auto_reconnect,
            cx,
        ))
        .child(paste_safety_card(
            paste_warn_multiline,
            paste_slow,
            paste_char_delay_ms,
            cx,
        ))
}

fn control_lines_card(
    dtr_on_connect: Entity<SelectState<Vec<Opt>>>,
    rts_on_connect: Entity<SelectState<Vec<Opt>>>,
    dtr_on_disconnect: Entity<SelectState<Vec<Opt>>>,
    rts_on_disconnect: Entity<SelectState<Vec<Opt>>>,
) -> gpui::Div {
    let row = |left_label, left, right_label, right| {
        div()
            .flex()
            .flex_row()
            .gap_3()
            .child(div().flex_1().child(labeled(left_label, left)))
            .child(div().flex_1().child(labeled(right_label, right)))
    };
    let body = div()
        .flex()
        .flex_col()
        .gap_3()
        .child(row(
            "DTR ON CONNECT",
            Select::new(&dtr_on_connect).small(),
            "RTS ON CONNECT",
            Select::new(&rts_on_connect).small(),
        ))
        .child(row(
            "DTR ON DISCONNECT",
            Select::new(&dtr_on_disconnect).small(),
            "RTS ON DISCONNECT",
            Select::new(&rts_on_disconnect).small(),
        ));
    section_card_with_desc(
        "Control Lines",
        Some(
            "Only needed for specific adapters or devices (RS-485 direction, \
             Arduino DTR-reset, firmwares that key off DTR for session lifecycle).",
        ),
        body,
    )
}

fn output_card(
    hex_view: bool,
    timestamps: bool,
    log_enabled: bool,
    auto_reconnect: bool,
    cx: &mut Context<AppView>,
) -> gpui::Div {
    // Single column. The 2-col grid for Hex view + Line timestamps
    // produced an awkward orphan-feel where the right-hand "Line
    // timestamps" hint wrapped while the others were full-width;
    // stacking all four reads cleaner and matches the rest of the
    // card's vertical rhythm.
    let body = div()
        .flex()
        .flex_col()
        .gap_2()
        .child(bool_field_hinted(
            "timestamps",
            "Line timestamps",
            Some("prefix each line with wall-clock time"),
            timestamps,
            cx,
            |ed, v| ed.timestamps = v,
        ))
        .child(bool_field_hinted(
            "hex-view",
            "Hex view",
            Some("show incoming bytes as hex dump"),
            hex_view,
            cx,
            |ed, v| ed.hex_view = v,
        ))
        .child(bool_field_hinted(
            "log-enabled",
            "Record session to file",
            Some("raw bytes; destination set in Settings → Advanced"),
            log_enabled,
            cx,
            |ed, v| ed.log_enabled = v,
        ))
        .child(bool_field_hinted(
            "auto-reconnect",
            "Auto-reconnect on drop",
            Some("poll for the port to reappear (up to 30s) and reopen transparently"),
            auto_reconnect,
            cx,
            |ed, v| ed.auto_reconnect = v,
        ));
    section_card("Output", body)
}

fn paste_safety_card(
    paste_warn_multiline: bool,
    paste_slow: bool,
    paste_char_delay_ms: Entity<InputState>,
    cx: &mut Context<AppView>,
) -> gpui::Div {
    // Slow-paste delay input gets its own row, indented under the
    // checkbox. Sharing a flex_row with the "Slow paste" hint made
    // the input visibly shrink as the window grew because the hint
    // text claimed more horizontal space at the input's expense.
    // 120px is enough for the largest sane delay (3 digits) plus
    // the small chevron padding the Input draws internally.
    let body = div()
        .flex()
        .flex_col()
        .gap_2()
        .child(bool_field_hinted(
            "paste-warn",
            "Confirm multi-line pastes",
            Some("prompt before sending pasted text that contains line breaks"),
            paste_warn_multiline,
            cx,
            |ed, v| ed.paste_warn_multiline = v,
        ))
        .child(bool_field_hinted(
            "paste-slow",
            "Slow paste",
            Some("send one char at a time with a delay"),
            paste_slow,
            cx,
            |ed, v| ed.paste_slow = v,
        ))
        .child(
            div()
                .pl_6()
                .w(px(160.0))
                .child(labeled(
                    "SLOW-PASTE DELAY (MS)",
                    Input::new(&paste_char_delay_ms).small().appearance(true),
                )),
        );
    section_card_with_desc(
        "Paste safety",
        Some(
            "Catch the \"I pasted into the wrong window\" mistake, and pace pastes so \
             UARTs on slower devices don't drop bytes.",
        ),
        body,
    )
}

/// Bright reddish-pink for the inline connect-error message
/// under a profile row. Bright enough to read on the dark sidebar
/// without being painful.
const SIDEBAR_ERROR: u32 = 0xff7a8a;
/// Live-connection status dot colour. Matches the Tauri version's
/// `--success` skin token (Baudrun palette) so the chrome agrees
/// across the two builds.
const STATUS_CONNECTED: u32 = 0x32d74b;
/// Failed-connection status dot colour. Reuses the inline-error
/// pink so the dot and the under-row error text agree visually.
const STATUS_FAILED: u32 = SIDEBAR_ERROR;
/// Auto-reconnect-in-progress dot colour. Matches the Tauri
/// version's `--warn` skin token (warm yellow), paired with a
/// pulse animation in `session_header` to draw the eye to the
/// "trying" state.
const STATUS_RECONNECTING: u32 = 0xf5d76e;
/// Diameter of the round status dot in the row header. 8px reads
/// at the sidebar's font size without crowding the name text.
const STATUS_DOT_PX: f32 = 8.0;

/// Per-row connection state used to paint the status dot. `None`
/// (in the caller) means no dot at all; the explicit variants keep
/// the row colour + animation table to a one-line lookup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RowStatus {
    Connected,
    Reconnecting,
    Failed,
}

impl RowStatus {
    fn color(self) -> u32 {
        match self {
            RowStatus::Connected => STATUS_CONNECTED,
            RowStatus::Reconnecting => STATUS_RECONNECTING,
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

    // Header row: name on the left, status dot on the right (omitted
    // when there's no status). Click anywhere on the row opens the
    // editor for that profile (or stays in the terminal view if
    // the row IS the connected profile) — see
    // `AppView::select_profile`. The standalone "edit" link the
    // sidebar used to carry is gone since the row click already
    // goes to the editor under the new flow.
    let header = div()
        .w_full()
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .child(div().text_color(rgb(SIDEBAR_FG)).child(name))
        .children(status.map(|s| {
            let dot = div()
                .w(px(STATUS_DOT_PX))
                .h(px(STATUS_DOT_PX))
                .rounded_full()
                .bg(rgb(s.color()));
            if s == RowStatus::Reconnecting {
                // Same 1s pulse as the session header so the two
                // indicators feel like one signal. Animation name
                // is per-row-instance to avoid gpui de-duping
                // animations across distinct dots.
                dot.with_animation(
                    SharedString::from(format!("sidebar-reconnect-pulse-{}", id)),
                    Animation::new(Duration::from_secs(1))
                        .repeat()
                        .with_easing(pulsating_between(0.35, 1.0)),
                    |el, delta| el.opacity(delta),
                )
                .into_any_element()
            } else {
                dot.into_any_element()
            }
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
