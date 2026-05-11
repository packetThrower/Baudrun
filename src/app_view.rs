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
    anchored, deferred, div, prelude::*, pulsating_between, px, rgba, Animation, AnimationExt,
    AppContext, Bounds, Context, Entity, IntoElement, MouseButton, MouseDownEvent, MouseUpEvent,
    PathPromptOptions, Pixels, Render, ScrollHandle, Task, TitlebarOptions, Window, WindowBounds,
    WindowHandle, WindowOptions,
};
use gpui_component::{
    checkbox::Checkbox,
    input::{Input, InputState},
    notification::Notification,
    scroll::ScrollableElement,
    select::{Select, SelectItem, SelectState},
    tooltip::Tooltip,
    Disableable, IndexPath, Root, Sizable, Theme, ThemeMode, WindowExt,
};
use gpui::SharedString;

use crate::data::appdata;
use crate::data::highlight;
use crate::data::transfer::{self, ChannelReader, XModemVariant};
use crate::serial_io::{ChannelWriter, TransferSink};
use crate::data::profiles::{self, Profile};
use crate::data::sanitize::SanitizingLogWriter;
use crate::data::serial::ports;
use crate::data::settings;
use crate::data::skins;
use crate::data::themes;
use crate::settings_bus::{SettingsBus, SettingsEvent};
use crate::settings_view::SettingsView;
use crate::skin_tokens::{SkinFonts, SkinTokens};
use crate::term_bridge::Palette;
use crate::serial_io;
use crate::terminal_view::{ProfileSettings, TerminalView};

/// Width of the left sidebar in logical pixels. Matches the main
/// app's sidebar width — wide enough for two-line profile rows
/// (name + port_name) without truncation on typical setups, narrow
/// enough that the terminal still gets the lion's share of the
/// window.
const SIDEBAR_WIDTH_PX: f32 = 220.0;

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
    /// Live system-appearance flag (`true` = dark mode). Seeded
    /// from `window.appearance()` at boot and updated by the
    /// `observe_window_appearance` subscription whenever the OS
    /// flips Light/Dark. Used by `apply_skin` when
    /// `Settings → Appearance → Auto` is picked so the chrome
    /// tracks the OS without a restart.
    system_dark: bool,
    /// Subscription handle for the OS appearance observer.
    /// Held to keep the callback alive for the AppView's
    /// lifetime; dropped on view teardown. Installed by
    /// `attach_appearance_observer` from the window builder
    /// (which has the &mut Window access the observer needs).
    _appearance_sub: Option<gpui::Subscription>,
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
    /// I/O handles needed to drive a file transfer over the live
    /// session: a clone of the write channel (so the transfer thread
    /// can push outbound bytes without contending with the terminal's
    /// keystroke writer) and the slot the read loop checks for
    /// inbound-byte diversion. `None` while disconnected.
    transfer_io: Option<TransferIo>,
    /// `Some(_)` while a file transfer is running. Holds the cancel
    /// atomic, the live progress counter, and the gpui task that
    /// polls them — dropping the option (in `finish_transfer`) is
    /// what tears the transfer down.
    transfer: Option<TransferState>,
    /// `Some(_)` while the Send File dialog is open. Lives outside
    /// the dialog because Choose…, Cancel, and Send all need to
    /// mutate the same widget state across re-renders. Cleared in
    /// `close_send_file_dialog` (Cancel / X / Send).
    send_file: Option<SendFileState>,
    /// `Some(_)` while the Send Hex dialog is open. Holds the live
    /// hex input plus a shared error mutex the dialog reads on
    /// every render to surface parse / send failures inline.
    send_hex: Option<SendHexState>,
    /// `true` while the session-header `⋯` overflow menu is open.
    /// Cleared by clicking an item, clicking outside (root mouse-
    /// up listener), or any other path that calls
    /// `dismiss_session_overflow`.
    session_overflow_open: bool,
    /// `Some(_)` while a profile-row right-click menu is showing.
    /// Carries the right-clicked profile id + the click position so
    /// the popup can render anchored at the cursor. Cleared by any
    /// click — menu items handle their action first; the root
    /// AppView listener clears state on the same mouse-up.
    profile_context_menu: Option<ProfileContextMenu>,
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
    /// Compact fingerprint of the highlight rule set last pushed
    /// to the terminal — `(pattern, color, ignore_case)` tuples
    /// in the order they were resolved. Used by `apply_highlight`
    /// to skip the (expensive) session replay when an unrelated
    /// settings change (skin, theme, log dir, …) fires the bus
    /// subscription. `None` means "nothing applied yet" so the
    /// first apply always runs.
    last_highlight_sig: Option<Vec<(String, String, bool)>>,
    /// `Some` while the new-profile form is open in the right pane.
    /// The presence of this field also drives a render branch:
    /// when populated the form replaces the TerminalView; when
    /// `None` the terminal is back. Holds `Entity<InputState>`s
    /// per field so the Input widgets persist their text + cursor
    /// across re-renders without us mirroring it into AppView.
    editor: Option<Editor>,
    /// `true` while the connected session is suspended — the port +
    /// OS threads stay alive (bytes still flow into TerminalView's
    /// scrollback), but the terminal viewport is hidden so the
    /// user can browse other profiles or settings without
    /// disconnecting. Cleared on disconnect / migration / explicit
    /// resume. Mirrors `App.svelte::suspended` in the Tauri build.
    suspended: bool,
}

/// In-flight profile form state. Created by `open_editor` (new) or
/// `open_editor_for` (existing). Both paths need `&mut Window`
/// because gpui-component's `InputState::new` hooks the window's
/// text-system at construction. Read by `save_editor` to materialize
/// a `Profile`. Dropped on cancel / successful save / successful
/// delete by setting `AppView::editor = None`.
/// Which sub-tab inside the form is currently active. Mirrors the
/// Tauri ProfileForm.svelte's left-rail.
#[derive(Clone, Copy, PartialEq, Eq)]
enum EditorTab {
    Connection,
    Highlighting,
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
    line_numbers: bool,
    log_enabled: bool,
    auto_reconnect: bool,

    // -- Highlighting tab --
    /// Master switch — when off, `apply_highlight` ignores the pack
    /// list entirely. Mirrors `Profile::highlight`.
    highlight: bool,
    /// `true` => use the per-pack list below; `false` => inherit the
    /// global Settings selection. Maps to `Some(_)` vs `None` on
    /// `Profile::enabled_highlight_presets` at save time.
    override_highlight_packs: bool,
    /// Profile-scoped pack id list. Only consulted when
    /// `override_highlight_packs` is true.
    enabled_highlight_packs: Vec<String>,

    /// Snapshot of unenrolled USB-serial adapters detected when the
    /// editor was opened. Rendered as a yellow banner above the
    /// Serial Port field so the user sees what's plugged in but not
    /// drivered. Re-evaluated each time the rescan button fires so
    /// the banner reflects current OS state.
    missing_drivers: Vec<crate::data::serial::chipsets::USBSerialCandidate>,

    // -- Advanced / Appearance --
    /// Per-profile theme override. The first option in the picker
    /// is `Opt::new("", "Use global default")`, with the empty id
    /// meaning "fall through to settings.default_theme_id" — same
    /// shape Tauri uses for `themeId`. Profiles ship with empty
    /// here from `Profile::defaults()` so users only opt into
    /// per-profile themes deliberately.
    theme: Entity<SelectState<Vec<Opt>>>,

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

/// I/O handles for the live serial session that the AppView needs
/// to drive features other than keystroke writes — file transfer
/// (write_tx clone + transfer_sink) and the Send Break / Send Hex
/// menu items (break_tx, write_tx). The kept-by-value name is
/// historical; despite its name, this struct now backs everything
/// the session toolbar's overflow menu hits.
struct TransferIo {
    write_tx: flume::Sender<Vec<u8>>,
    break_tx: flume::Sender<std::time::Duration>,
    transfer_sink: std::sync::Arc<std::sync::Mutex<Option<TransferSink>>>,
}

/// In-flight file transfer state. The poll task, the cancel atomic,
/// and the live progress counter all live here; dropping the option
/// is what tears the transfer down (the `Task` cancels, the dialog
/// stops being rebuilt, and `clear_transfer_sink` was already called
/// by the transfer thread on its way out).
struct TransferState {
    filename: SharedString,
    /// Total bytes the protocol thread will push. Set once at start.
    total: u64,
    /// Bytes ACKed so far. Updated by the protocol's progress
    /// callback; read on each frame by the dialog renderer. Atomic
    /// (not Mutex) so the renderer doesn't block the protocol thread.
    sent: std::sync::Arc<std::sync::atomic::AtomicU64>,
    /// Set by `cancel_transfer`; observed by `send_xmodem`/
    /// `send_ymodem` between blocks. Same Arc the protocol thread
    /// holds via `Options::cancel`.
    cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
    /// Drains the result channel and forces a re-render every tick
    /// so the progress dialog updates without us hooking the
    /// protocol thread directly into gpui. Dropped automatically
    /// when `transfer` is cleared.
    _poll_task: Task<()>,
}

#[derive(Debug)]
enum TransferResult {
    Ok,
    Err(String),
}

/// Direction for the font-size shortcuts. `Increase` / `Decrease`
/// bump by one px (clamped 8..=48); `Reset` snaps back to the
/// `terminal_grid::FONT_SIZE_PX` boot default.
#[derive(Debug, Clone, Copy)]
enum FontBump {
    Increase,
    Decrease,
    Reset,
}

/// Right-click menu state for sidebar profile rows. Stores the
/// profile id that was right-clicked plus the cursor position to
/// anchor the popup to.
struct ProfileContextMenu {
    profile_id: String,
    pos: gpui::Point<gpui::Pixels>,
}

/// Open-Send-Hex-dialog widget state. The Input is a gpui entity the
/// dialog can render directly; the error is a shared mutex the
/// dialog reads on every render so parse failures surface inline
/// without needing a re-open of the dialog.
struct SendHexState {
    input: Entity<InputState>,
    error: std::sync::Arc<std::sync::Mutex<Option<String>>>,
}

/// Open-Send-File-dialog widget state. The Select is a gpui entity
/// the Dialog can render directly without going through AppView.
/// The path lives behind an `Arc<Mutex<…>>` so the Dialog builder
/// can re-read it on each frame without leasing the AppView entity
/// — gpui-component's dialog layer runs the builder from inside
/// AppView's render path, where re-leasing AppView would panic.
struct SendFileState {
    protocol: Entity<SelectState<Vec<Opt>>>,
    selected_path: std::sync::Arc<std::sync::Mutex<Option<std::path::PathBuf>>>,
}

impl AppView {
    pub fn new(
        terminal: Entity<TerminalView>,
        profile_store: Rc<profiles::Store>,
        settings_bus: Entity<SettingsBus>,
        skins_store: Rc<skins::Store>,
        highlight_store: Rc<highlight::Store>,
        themes_store: Rc<themes::Store>,
        system_dark: bool,
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
            system_dark,
            _appearance_sub: None,
            settings_window: None,
            selected_profile_id: None,
            connected_profile_id: None,
            connect_error: None,
            drain_task: None,
            serial_disconnect: None,
            transfer_io: None,
            transfer: None,
            send_file: None,
            send_hex: None,
            session_overflow_open: false,
            profile_context_menu: None,
            auto_reconnect_task: None,
            auto_reconnect_for: None,
            last_highlight_sig: None,
            editor: None,
            suspended: false,
        };
        this.apply_settings(&initial, cx);
        this
    }

    /// Apply the relevant slots of a `Settings` snapshot to the
    /// live UI. Called both at construction time and on every
    /// `SettingsEvent::Updated` from the bus. Refreshes:
    ///  * the chrome `SkinTokens` global (drives both windows'
    ///    sidebar / panel / button colours),
    ///  * the terminal palette (honours per-profile theme override
    ///    over the global), and
    ///  * the live highlight rule set when there's an active
    ///    session — toggling packs in Settings → Highlighting
    ///    re-coloures incoming bytes without a reconnect.
    fn apply_settings(&mut self, settings: &settings::Settings, cx: &mut Context<Self>) {
        self.apply_skin(settings, cx);
        self.apply_palette(cx);
        self.apply_highlight(cx);
        self.apply_font_size(settings, cx);
        self.apply_scrollback(settings, cx);
    }

    fn apply_scrollback(
        &mut self,
        settings: &settings::Settings,
        cx: &mut Context<Self>,
    ) {
        let lines = settings.effective_scrollback();
        self.terminal
            .update(cx, |t, cx| t.set_scrollback_lines(lines, cx));
    }

    /// Push the user's terminal font size to the view. Empty / 0
    /// means "use the boot default" — the live UI sometimes saves
    /// 0 when the field is cleared (the JSON's
    /// `skip_serializing_if = "is_zero"` represents the same
    /// state). Outside a sane range gets clamped so a stray edit
    /// doesn't render unreadable text.
    fn apply_font_size(
        &mut self,
        settings: &settings::Settings,
        cx: &mut Context<Self>,
    ) {
        let raw = settings.font_size;
        let size = if raw <= 0 {
            crate::terminal_grid::FONT_SIZE_PX
        } else {
            (raw as f32).clamp(8.0, 48.0)
        };
        self.terminal
            .update(cx, |t, cx| t.set_font_size(size, cx));
    }

    /// Hook the OS appearance observer onto the main window. Called
    /// by the open-window builder right after AppView is constructed
    /// — that's the only place where `&mut Window` is in scope long
    /// enough to register the callback. Each callback fire updates
    /// `system_dark` and, if the user is on `Auto`, re-applies the
    /// skin so the chrome flips Light/Dark live.
    pub fn attach_appearance_observer(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let weak = cx.entity().downgrade();
        let sub = window.observe_window_appearance(move |window, app_cx| {
            let dark = matches!(
                window.appearance(),
                gpui::WindowAppearance::Dark | gpui::WindowAppearance::VibrantDark
            );
            if let Some(strong) = weak.upgrade() {
                strong
                    .update(app_cx, |this, view_cx| {
                        this.set_system_dark(dark, view_cx);
                    });
            }
        });
        self._appearance_sub = Some(sub);
    }

    /// Update the cached OS-appearance flag and re-apply chrome
    /// only if the user's pick is `Auto` (so an explicit Light /
    /// Dark choice doesn't get clobbered by an OS swap).
    fn set_system_dark(&mut self, dark: bool, cx: &mut Context<Self>) {
        if self.system_dark == dark {
            return;
        }
        self.system_dark = dark;
        let settings = self.settings_bus.read(cx).current().clone();
        let on_auto = settings.appearance.is_empty()
            || settings.appearance.as_str() == "auto";
        if on_auto {
            self.apply_settings(&settings, cx);
            cx.notify();
        }
    }

    /// Push the effective highlight rule set to the terminal for
    /// whichever profile is currently connected (or auto-
    /// reconnecting). No-op when there's no active session — the
    /// next `connect_to` will resolve fresh rules at that point.
    fn apply_highlight(&mut self, cx: &mut Context<Self>) {
        let active_id = self
            .connected_profile_id
            .as_deref()
            .or(self.auto_reconnect_for.as_deref());
        let Some(id) = active_id else { return };
        let Some(profile) = self.profile_store.get(id) else {
            return;
        };
        let rules = if profile.highlight {
            Some(self.compute_highlight_rules(&profile, cx))
        } else {
            None
        };
        // Short-circuit when the resolved rules match what we
        // last applied: every settings change fires this path
        // (skin/theme/log dir/...) and the session replay is
        // expensive enough that we want to skip it for any
        // edit that doesn't actually move the highlight set.
        let sig = rules.as_ref().map(|rs| {
            rs.iter()
                .map(|r| (r.pattern.clone(), r.color.clone(), r.ignore_case))
                .collect::<Vec<_>>()
        });
        if self.last_highlight_sig == sig {
            return;
        }
        self.last_highlight_sig = sig;
        self.terminal
            .update(cx, |t, cx| t.set_highlight_rules(rules, cx));
    }

    /// Resolve the active skin and write the chrome tokens into
    /// the gpui `Global`. Both AppView and SettingsView read from
    /// that global during render, so a single update propagates
    /// to every chrome surface — no need to push tokens through
    /// every helper signature. Also re-tunes the gpui-component
    /// `Theme` (mode + input/popover colours + fonts) so widgets
    /// (Select, Input, Checkbox) match the active skin instead of
    /// staying frozen at boot-time defaults.
    fn apply_skin(&mut self, settings: &settings::Settings, cx: &mut Context<Self>) {
        let skin_id = if settings.skin_id.is_empty() {
            skins::DEFAULT_SKIN_ID
        } else {
            settings.skin_id.as_str()
        };
        let (skin_opt, fallback_dark) = match self.skins_store.get(skin_id) {
            Some(skin) => (Some(skin), false),
            None => {
                log::warn!("skin {skin_id:?} not found, using built-in baudrun");
                (None, true)
            }
        };
        // Resolve the user's appearance pick into a concrete
        // dark/light boolean:
        //   `"light"` → light
        //   `"dark"`  → dark
        //   anything else (`"auto"`, empty, …) → follow the OS
        // Skins flagged dark-only ignore the request and pin dark,
        // matching the Tauri applier.
        let user_dark = match settings.appearance.as_str() {
            "light" => false,
            "dark" => true,
            _ => self.system_dark,
        };
        let dark = match &skin_opt {
            Some(skin) => user_dark || !skin.supports_light,
            None => fallback_dark || user_dark,
        };
        let (tokens, fonts) = match &skin_opt {
            Some(skin) => (
                SkinTokens::from_skin(skin, dark),
                SkinFonts::from_skin(skin, dark),
            ),
            None => (SkinTokens::baudrun_default(), SkinFonts::defaults()),
        };
        cx.set_global(tokens);

        // Drive gpui-component's Theme off the same appearance
        // signal so Select dropdowns / Inputs / Checkboxes pick the
        // matching light/dark variant of their own theme. Without
        // this the widgets stay in whatever mode `Theme::change`
        // was last called with, so picking Light leaves them
        // showing white-on-white text.
        let mode = if dark { ThemeMode::Dark } else { ThemeMode::Light };
        Theme::change(mode, None, cx);

        // Now overlay the skin's colour + size tokens onto the
        // Theme so the widget chrome (input border, popover bg,
        // accent, radii, fonts) tracks the picked skin rather than
        // the gpui-component defaults. Done after `Theme::change`
        // so the mode swap doesn't clobber our overrides on the
        // next change.
        let theme = Theme::global_mut(cx);
        theme.input = rgba(tokens.border_strong).into();
        // gpui-component renders BOTH the closed Select box (the
        // input chrome) AND the open Select popover with
        // `theme.background`. The popover floats free over the
        // window so a translucent value (Baudrun + macOS-26's
        // `--bg-main` is alpha 0.55-0.7) lets the cards underneath
        // bleed through. Use the same opaque `--option-bg` slot
        // the popover items themselves use — the closed Select box
        // ends up slightly more "raised" than before but agrees
        // visually with the open popover.
        theme.background = rgba(tokens.option_bg).into();
        theme.foreground = rgba(tokens.fg_primary).into();
        // Popover (Select dropdown menu) uses the dedicated
        // `--option-bg` / `--option-fg` slots — these are opaque in
        // every shipped skin, since the popover floats free over the
        // window and would otherwise let whatever's behind the
        // window bleed through. Using `bg_panel` (translucent in
        // Baudrun + macOS-26) made the dropdown look glassy.
        theme.popover = rgba(tokens.option_bg).into();
        theme.popover_foreground = rgba(tokens.option_fg).into();
        // gpui-component's Select hardcodes selected-option text
        // to `theme.foreground` and the selected-option bg to
        // `theme.accent` — meaning a saturated accent shows the
        // foreground colour right on top of itself (green-on-green
        // for CRT, magenta-on-magenta for Synthwave). Skins ship
        // `--bg-active` as a translucent version of the accent
        // tuned for exactly this "selected row" use case;
        // composited over the popover bg it always lands at a
        // muted shade where the foreground reads. Keep the
        // saturated `accent` token for the primary Connect button
        // (which has its own accent_fg contrast pair) and use
        // `bg_active` for the Theme accent slot that drives Select
        // highlighting.
        theme.accent = rgba(tokens.bg_active).into();
        theme.accent_foreground = rgba(tokens.option_fg).into();
        // gpui-component's Checkbox paints both border + filled box
        // from `theme.primary` when checked, and the inner check
        // glyph from `theme.primary_foreground`. Pinning both to the
        // skin's saturated accent (the same colour the Connect pill
        // button uses) keeps the toggled-on checkboxes visually
        // consistent with the rest of the primary affordances.
        theme.primary = rgba(tokens.accent).into();
        theme.primary_foreground = rgba(tokens.accent_fg).into();
        theme.primary_hover = rgba(tokens.accent).into();
        theme.primary_active = rgba(tokens.accent).into();
        theme.danger = rgba(tokens.danger).into();
        theme.border = rgba(tokens.border_subtle).into();
        theme.radius = px(tokens.radius_md);
        theme.radius_lg = px(tokens.radius_lg);
        if !fonts.font_ui.is_empty() {
            theme.font_family = fonts.font_ui.clone();
        }
        if !fonts.font_mono.is_empty() {
            theme.mono_font_family = fonts.font_mono.clone();
        }
        theme.font_size = px(fonts.font_size_base);
    }

    /// Resolve the effective palette right now and push it to the
    /// terminal. Resolution honours the precedence the Tauri side
    /// established: the connected profile's `theme_id` override
    /// wins, falling back to the global `default_theme_id`, then
    /// the built-in Baudrun palette if both lookups miss. Called
    /// after every `connect_to` (so a profile-scoped theme takes
    /// effect on connect) and after every settings update (so a
    /// global change applies live unless an override is shadowing
    /// it).
    fn apply_palette(&mut self, cx: &mut Context<Self>) {
        let palette = self.compute_palette(cx);
        self.terminal
            .update(cx, |term, cx| term.set_palette(palette, cx));
    }

    /// Resolve the effective highlight rule set for a profile —
    /// per-profile `enabled_highlight_presets` override wins, else
    /// the global Settings list, else `Settings::default()`'s pick
    /// of `["baudrun-default", "user"]`. The flat `Vec<HighlightRule>`
    /// concatenates each enabled pack's rules in store order;
    /// first-match-wins precedence is enforced inside
    /// `HighlightEngine::apply` per line, not here.
    fn compute_highlight_rules(
        &self,
        profile: &Profile,
        cx: &mut Context<Self>,
    ) -> Vec<crate::data::highlight::HighlightRule> {
        let global = self.settings_bus.read(cx).current().clone();
        let enabled_ids = profile
            .enabled_highlight_presets
            .clone()
            .or(global.enabled_highlight_presets)
            .unwrap_or_else(|| {
                settings::Settings::default()
                    .enabled_highlight_presets
                    .unwrap_or_default()
            });
        let mut out = Vec::new();
        for pack in self.highlight_store.list() {
            if enabled_ids.iter().any(|id| id == &pack.id) {
                out.extend(pack.rules.iter().cloned());
            }
        }
        out
    }

    /// Pure resolver — reads from the bus + stores, no mutation.
    /// Treats `auto_reconnect_for` the same as `connected_profile_id`
    /// for the override check: a transient drop shouldn't yank the
    /// terminal back to the global palette mid-retry.
    fn compute_palette(&self, cx: &mut Context<Self>) -> Palette {
        let active_profile_id = self
            .connected_profile_id
            .as_deref()
            .or(self.auto_reconnect_for.as_deref());
        if let Some(id) = active_profile_id {
            if let Some(profile) = self.profile_store.get(id) {
                if !profile.theme_id.is_empty() {
                    if let Some(theme) = self.themes_store.get(&profile.theme_id) {
                        return Palette::from_theme(&theme);
                    }
                    log::warn!(
                        "profile theme {:?} not found, falling back to global",
                        profile.theme_id
                    );
                }
            }
        }
        let settings = self.settings_bus.read(cx).current();
        let theme_id = if settings.default_theme_id.is_empty() {
            themes::DEFAULT_THEME_ID
        } else {
            settings.default_theme_id.as_str()
        };
        match self.themes_store.get(theme_id) {
            Some(theme) => Palette::from_theme(&theme),
            None => {
                log::warn!("theme {theme_id:?} not found, using built-in baudrun");
                Palette::baudrun()
            }
        }
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
            // Clicking the connected row while suspended is the
            // natural "I want to see the terminal again" gesture —
            // resume implicitly so the next paint shows the
            // viewport instead of falling through to a stale
            // editor / welcome pane.
            self.suspended = false;
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
        // Pull off the transfer-side handles before destructuring
        // `channels` further. The terminal swallows `write_tx`; we
        // hand the transfer thread its own clone so the two writers
        // (keystrokes and protocol bytes) can't deadlock each other.
        let write_tx = channels.write_tx.clone();
        let transfer_write = channels.write_tx;
        let transfer_sink = channels.transfer_sink.clone();
        let break_tx = channels.break_tx;
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
            line_numbers: profile.line_numbers,
        };
        // Resolve the effective highlight rule set for this
        // profile + push it to the terminal. Master toggle is
        // `profile.highlight`; the enabled-pack list is per-
        // profile if the profile overrode, else global.
        let highlight_rules = if profile.highlight {
            Some(self.compute_highlight_rules(&profile, cx))
        } else {
            None
        };
        // Cache the signature so the bus subscription's
        // apply_highlight short-circuits when an unrelated
        // settings change fires after this connect.
        self.last_highlight_sig = highlight_rules.as_ref().map(|rs| {
            rs.iter()
                .map(|r| (r.pattern.clone(), r.color.clone(), r.ignore_case))
                .collect()
        });
        self.terminal.update(cx, |t, cx| {
            t.set_serial_tx(write_tx);
            t.set_profile_settings(settings);
            t.set_highlight_rules(highlight_rules, cx);
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
        self.transfer_io = Some(TransferIo {
            write_tx: transfer_write,
            break_tx,
            transfer_sink,
        });
        self.connected_profile_id = Some(profile.id);
        // A successful (re)connect ends any auto-reconnect window —
        // the right pane should now render off the live session, not
        // the placeholder stand-in that kept the terminal visible
        // while we were polling.
        self.auto_reconnect_for = None;
        // Clear any stale failure message from a prior attempt
        // (e.g. earlier ticks of the auto-reconnect retry loop).
        self.connect_error = None;
        // Re-resolve the palette now that `connected_profile_id` is
        // set — `compute_palette` will pick up this profile's
        // `theme_id` override (if any) and shadow the global default.
        self.apply_palette(cx);
    }

    /// Open the form for a new profile, seeded from `Profile::defaults`.
    /// Idempotent if already open — re-creates the field state so the
    /// user gets a fresh form rather than whatever they typed before.
    fn open_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let detect = !self.settings_bus.read(cx).current().disable_driver_detection;
        self.editor = Some(build_editor(
            None,
            &Profile::defaults(),
            &self.themes_store,
            detect,
            window,
            cx,
        ));
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
        let detect = !self.settings_bus.read(cx).current().disable_driver_detection;
        self.editor = Some(build_editor(
            Some(id),
            &profile,
            &self.themes_store,
            detect,
            window,
            cx,
        ));
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
    pub(crate) fn disconnect_current(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.connected_profile_id.take() else {
            return;
        };
        // Disconnecting always exits the suspended state — the port
        // is gone, there's nothing to resume back to.
        self.suspended = false;
        // Cancel any in-flight transfer first — flipping the cancel
        // atomic lets `send_xmodem` exit on the next block boundary
        // instead of stalling on `next_byte` after the read thread
        // dies. We then drop the state itself, which closes the
        // dialog and stops the poll task.
        if let Some(t) = self.transfer.as_ref() {
            t.cancel.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        self.transfer = None;
        self.transfer_io = None;
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

    /// Flip the override flag on the editor's highlight section.
    /// Turning override ON seeds the per-profile pack list with
    /// whatever the global currently has, so the user sees the same
    /// active set they were inheriting and can edit from there
    /// rather than starting empty. Turning it OFF clears the list
    /// (`apply_editor_to_profile` collapses to `None` anyway).
    fn set_editor_override_highlight(&mut self, on: bool, cx: &mut Context<Self>) {
        let Some(ed) = self.editor.as_mut() else { return };
        if ed.override_highlight_packs == on {
            return;
        }
        ed.override_highlight_packs = on;
        if on && ed.enabled_highlight_packs.is_empty() {
            // Seed with the current global pick so the override
            // doesn't silently drop highlighting on first toggle.
            let global = self.settings_bus.read(cx).current().clone();
            ed.enabled_highlight_packs = global
                .enabled_highlight_presets
                .unwrap_or_else(|| {
                    settings::Settings::default()
                        .enabled_highlight_presets
                        .unwrap_or_default()
                });
        }
        cx.notify();
    }

    /// Toggle a single pack id on the editor's per-profile list.
    fn toggle_editor_highlight_pack(
        &mut self,
        id: String,
        on: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(ed) = self.editor.as_mut() else { return };
        let already = ed.enabled_highlight_packs.iter().any(|p| p == &id);
        if on && !already {
            ed.enabled_highlight_packs.push(id);
        } else if !on && already {
            ed.enabled_highlight_packs.retain(|p| p != &id);
        } else {
            return;
        }
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
    pub(crate) fn open_settings(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
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
        let current_settings = self.settings_bus.read(cx).current().clone();
        let restore_state = !current_settings.disable_window_state_restore;
        // First-launch default: smaller window than the cards' max
        // intrinsic width so the right column doesn't render a sea
        // of empty space. Saved geometry from a previous session
        // (when restore-window-state is on) trumps this; the user
        // can always drag the window bigger and that size sticks.
        let bounds = restore_state
            .then(|| current_settings.settings_window.as_ref().and_then(geometry_to_bounds))
            .flatten()
            .unwrap_or_else(|| {
                Bounds::centered(None, gpui::size(px(560.0), px(520.0)), cx)
            });
        let bus_for_close = self.settings_bus.clone();
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
                window.on_window_should_close(cx, move |window, cx| {
                    let geom = bounds_to_geometry(window.bounds());
                    bus_for_close.update(cx, |bus, cx| {
                        let mut next = bus.current().clone();
                        if !next.disable_window_state_restore {
                            next.settings_window = Some(geom);
                            if let Err(err) = bus.replace(next, cx) {
                                log::error!("save settings-window state: {err}");
                            }
                        }
                    });
                    true
                });
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

    /// Toggle the session-header `⋯` overflow menu. Single source
    /// of truth so both the button click and the dismiss-on-outside
    /// listener can drive the state.
    fn toggle_session_overflow(&mut self, cx: &mut Context<Self>) {
        self.session_overflow_open = !self.session_overflow_open;
        cx.notify();
    }

    fn dismiss_session_overflow(&mut self, cx: &mut Context<Self>) {
        if self.session_overflow_open {
            self.session_overflow_open = false;
            cx.notify();
        }
    }

    /// Fire a serial break (`set_break` / sleep / `clear_break`)
    /// down the live write channel. Surfaces success / failure via
    /// toast since there's no inline UI affordance for it.
    pub(crate) fn send_break_now(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.session_overflow_open = false;
        let Some(io) = self.transfer_io.as_ref() else {
            return;
        };
        let result = io.break_tx.send(serial_io::BREAK_DURATION);
        cx.spawn(async move |this, cx| {
            let _ = this.update_in(cx, |_, window, cx| match result {
                Ok(()) => {
                    window.push_notification(
                        Notification::success(SharedString::from("Break sent")),
                        cx,
                    );
                }
                Err(err) => {
                    log::error!("send break: {err}");
                    window.push_notification(
                        Notification::error(SharedString::from(format!(
                            "Couldn't send break: {err}"
                        ))),
                        cx,
                    );
                }
            });
        })
        .detach();
    }

    /// Open the Send Hex dialog. Builds an Input entity + the shared
    /// error mutex, opens a gpui-component Dialog wired against
    /// both. Cancel / Send / X close the dialog via
    /// `close_send_hex` (drops state) and `submit_send_hex`
    /// (parses + sends bytes + closes on success).
    fn open_send_hex(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.session_overflow_open = false;
        if self.transfer_io.is_none() {
            window.push_notification(
                Notification::error(SharedString::from(
                    "Connect to a port before sending hex.",
                )),
                cx,
            );
            return;
        }
        if self.send_hex.is_some() {
            return;
        }
        let input = cx.new(|cx| {
            InputState::new(window, cx).placeholder("02 FF AA 55")
        });
        let error: std::sync::Arc<std::sync::Mutex<Option<String>>> =
            std::sync::Arc::new(std::sync::Mutex::new(None));
        self.send_hex = Some(SendHexState {
            input: input.clone(),
            error: error.clone(),
        });

        let app = cx.entity();
        window.open_dialog(cx, move |dlg, _, _| {
            let app_close = app.clone();
            let app_send = app.clone();
            let app_cancel = app.clone();
            let error_for_render = error.clone();
            let live_err = error_for_render.lock().unwrap().clone();
            dlg.title(SharedString::from("Send hex bytes"))
                .w(px(560.0))
                .close_button(true)
                .on_close(move |_, _, cx| {
                    app_close.update(cx, |this, cx| this.close_send_hex(cx));
                })
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .text_size(px(13.0))
                        .child(
                            div().text_color(rgba(0x808080AAu32)).child(
                                "Space-separated, compact, or 0x-prefixed — all \
                                 equivalent: 02 FF AA 55, 02FFAA55, \
                                 0x02 0xFF 0xAA 0x55.",
                            ),
                        )
                        .child(Input::new(&input).appearance(true))
                        .children(live_err.map(|err| {
                            div()
                                .text_size(px(12.0))
                                .text_color(rgba(0xCC4444FFu32))
                                .child(SharedString::from(format!(
                                    "Invalid: {err}"
                                )))
                        }))
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .justify_end()
                                .gap_2()
                                .child(send_file_secondary_button(
                                    "Cancel",
                                    move |_, window, cx| {
                                        let _ = &app_cancel;
                                        window.close_dialog(cx);
                                    },
                                ))
                                .child(send_file_primary_button(
                                    "Send",
                                    true,
                                    move |_, window, cx| {
                                        let app = app_send.clone();
                                        app.update(cx, |this, cx| {
                                            this.submit_send_hex(window, cx);
                                        });
                                    },
                                )),
                        ),
                )
        });
    }

    fn close_send_hex(&mut self, cx: &mut Context<Self>) {
        if self.send_hex.take().is_some() {
            cx.notify();
        }
    }

    /// Parse the input as hex, send the resulting bytes through the
    /// write channel, and either toast + close (success) or stash
    /// the error in the shared mutex and re-render (failure). Same
    /// rule as Tauri: strip `0x`, whitespace, commas; require an
    /// even count of hex digits.
    fn submit_send_hex(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(state) = self.send_hex.as_ref() else {
            return;
        };
        let raw = state.input.read(cx).value().to_string();
        match parse_hex_string(&raw) {
            Ok(bytes) if bytes.is_empty() => {
                *state.error.lock().unwrap() = Some("empty".into());
                cx.notify();
            }
            Ok(bytes) => {
                let Some(io) = self.transfer_io.as_ref() else {
                    *state.error.lock().unwrap() =
                        Some("not connected".into());
                    cx.notify();
                    return;
                };
                let count = bytes.len();
                if let Err(err) = io.write_tx.send(bytes) {
                    *state.error.lock().unwrap() = Some(err.to_string());
                    cx.notify();
                    return;
                }
                self.send_hex = None;
                window.close_dialog(cx);
                window.push_notification(
                    Notification::success(SharedString::from(format!(
                        "Sent {count} byte{}",
                        if count == 1 { "" } else { "s" }
                    ))),
                    cx,
                );
            }
            Err(msg) => {
                *state.error.lock().unwrap() = Some(msg.to_string());
                cx.notify();
            }
        }
    }

    /// "Send File…" button entry point. Builds the dialog state
    /// (protocol Select pre-selected to YMODEM, no path yet) and
    /// opens the modal. The dialog itself is rendered from
    /// `Render::render` reading `self.send_file` so the live state
    /// (chosen path, protocol) updates as the user picks them.
    pub(crate) fn start_send_file(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.transfer.is_some() || self.send_file.is_some() {
            return;
        }
        if self.transfer_io.is_none() {
            window.push_notification(
                Notification::error(SharedString::from(
                    "Connect to a port before sending a file.",
                )),
                cx,
            );
            return;
        }

        let opts = transfer_protocol_opts();
        // Default to YMODEM — matches the Tauri version + tends to
        // be what most modern bootloaders speak.
        let protocol = make_select(opts, YMODEM_PROTOCOL_ID, window, cx);
        let selected_path: std::sync::Arc<std::sync::Mutex<Option<std::path::PathBuf>>> =
            std::sync::Arc::new(std::sync::Mutex::new(None));
        self.send_file = Some(SendFileState {
            protocol: protocol.clone(),
            selected_path: selected_path.clone(),
        });

        let app = cx.entity();
        // Capture clones for each closure; dialog builder runs from
        // inside AppView's render path so we MUST NOT read the
        // AppView entity from inside it.
        window.open_dialog(cx, move |dlg, _, _| {
            let path_for_render = selected_path.clone();
            let path_now = path_for_render.lock().unwrap().clone();
            let path_label = path_now
                .as_ref()
                .map(|p| {
                    p.file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| p.display().to_string())
                })
                .unwrap_or_default();
            let send_enabled = path_now.is_some();
            let app_close = app.clone();
            let app_cancel = app.clone();
            let app_choose = app.clone();
            let app_send = app.clone();

            dlg.title(SharedString::from("Send file"))
                .w(px(560.0))
                .close_button(true)
                .on_close(move |_, _, cx| {
                    app_close.update(cx, |this, cx| this.close_send_file_dialog(cx));
                })
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .text_size(px(13.0))
                        .child(send_file_field_label("Protocol"))
                        .child(Select::new(&protocol))
                        .child(send_file_field_label("File"))
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap_2()
                                .child(send_file_path_pill(path_label))
                                .child(send_file_choose_button(move |_evt, window, cx| {
                                    let app = app_choose.clone();
                                    app.update(cx, |this, cx| {
                                        this.send_file_choose(window, cx);
                                    });
                                })),
                        )
                        .child(
                            div()
                                .text_size(px(12.0))
                                .text_color(rgba(0x808080AAu32))
                                .whitespace_normal()
                                .child(
                                    "Start the receiver on the target device first \
                                     (rx, loady, bootloader \u{201C}Receive File\u{201D} \
                                     menu, etc.) before clicking Send. The transfer \
                                     waits up to 60 s for the receiver's handshake \
                                     before giving up.",
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .justify_end()
                                .gap_2()
                                .child(send_file_secondary_button(
                                    "Cancel",
                                    move |_, window, cx| {
                                        // Dismissing the overlay triggers
                                        // `on_close` which clears state.
                                        let _ = &app_cancel;
                                        window.close_dialog(cx);
                                    },
                                ))
                                .child(send_file_primary_button(
                                    "Send",
                                    send_enabled,
                                    move |_, window, cx| {
                                        let app = app_send.clone();
                                        app.update(cx, |this, cx| {
                                            this.send_file_confirm(window, cx);
                                        });
                                    },
                                )),
                        ),
                )
        });
    }

    /// Spawn the transfer thread and the gpui poll task, install the
    /// read-side sink, open the progress dialog. Called once per
    /// chosen protocol from the picker buttons.
    fn kick_off_transfer(
        &mut self,
        path: std::path::PathBuf,
        variant: Option<XModemVariant>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Close the protocol picker first — it sits on top of the
        // progress dialog we're about to open.
        window.close_dialog(cx);

        let Some(io) = self.transfer_io.as_ref() else { return };

        let data = match std::fs::read(&path) {
            Ok(d) => d,
            Err(err) => {
                window.push_notification(
                    Notification::error(SharedString::from(format!(
                        "Couldn't read file: {err}"
                    ))),
                    cx,
                );
                return;
            }
        };
        let total = data.len() as u64;
        let filename: SharedString = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "(unknown)".into())
            .into();

        // Inbound bytes during the transfer flow through this
        // dedicated channel — the read thread's sink shoves chunks
        // in, the ChannelReader pops single bytes out. Bounded would
        // be a footgun (transfer thread blocks on a slow protocol)
        // so unbounded matches `serial_io`'s read path.
        let (in_tx, in_rx) = std::sync::mpsc::channel::<Vec<u8>>();
        let sink: TransferSink = Box::new(move |chunk: &[u8]| {
            let _ = in_tx.send(chunk.to_vec());
        });
        io.transfer_sink.lock().unwrap().replace(sink);

        let sent = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
        let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let (result_tx, result_rx) = flume::bounded::<TransferResult>(1);

        let progress_arc = sent.clone();
        let progress_fn: transfer::ProgressFn =
            std::sync::Arc::new(move |s, _t| {
                progress_arc.store(s, std::sync::atomic::Ordering::Relaxed);
            });
        let opts = transfer::Options {
            progress: Some(progress_fn),
            cancel: Some(cancel.clone()),
        };

        let writer_tx = io.write_tx.clone();
        let sink_slot = io.transfer_sink.clone();
        let filename_for_thread = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "file.bin".into());
        std::thread::Builder::new()
            .name("transfer-driver".into())
            .spawn(move || {
                let mut reader = ChannelReader::new(in_rx);
                let mut writer = ChannelWriter::new(writer_tx);
                let result = match variant {
                    Some(v) => {
                        transfer::send_xmodem(&mut reader, &mut writer, &data, v, &opts)
                    }
                    None => transfer::send_ymodem(
                        &mut reader,
                        &mut writer,
                        &filename_for_thread,
                        &data,
                        &opts,
                    ),
                };
                // Always restore the read path before signalling
                // done — otherwise the next inbound byte might land
                // on a stale sink whose receiver is gone.
                *sink_slot.lock().unwrap() = None;
                let payload = match result {
                    Ok(()) => TransferResult::Ok,
                    Err(err) => TransferResult::Err(err.to_string()),
                };
                let _ = result_tx.send(payload);
            })
            .expect("spawn transfer driver thread");

        // Poll task: re-renders AppView each tick (so the progress
        // dialog updates) and watches the result channel. When the
        // transfer thread sends a result we tear the state down and
        // push the success/error toast.
        let result_rx_for_task = result_rx.clone();
        let poll_task = cx.spawn(async move |this, cx| {
            let tick = std::time::Duration::from_millis(100);
            loop {
                cx.background_executor().timer(tick).await;
                if let Ok(payload) = result_rx_for_task.try_recv() {
                    let _ = this.update_in(cx, |this, window, cx| {
                        this.finish_transfer(payload, window, cx);
                    });
                    break;
                }
                let still = this
                    .update(cx, |this, cx| {
                        cx.notify();
                        this.transfer.is_some()
                    })
                    .unwrap_or(false);
                if !still {
                    break;
                }
            }
        });

        let sent_for_dialog = sent.clone();
        let app = cx.entity();
        let filename_for_dialog = filename.clone();
        window.open_dialog(cx, move |dlg, _, _| {
            let s_now = sent_for_dialog.load(std::sync::atomic::Ordering::Relaxed);
            let pct = if total > 0 {
                ((s_now as f64 / total as f64) * 100.0).min(100.0) as u32
            } else {
                0
            };
            let app = app.clone();
            dlg.title(SharedString::from(format!(
                "Sending \u{201C}{filename_for_dialog}\u{201D}"
            )))
            .w(px(420.0))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .justify_between()
                            .text_size(px(12.0))
                            .text_color(rgba(0x808080CCu32))
                            .child(div().child(format!("{} / {} bytes", s_now, total)))
                            .child(div().child(format!("{}%", pct))),
                    )
                    // Bar — outer track + inner fill scaled to %.
                    .child(
                        div()
                            .w_full()
                            .h(px(6.0))
                            .rounded(px(3.0))
                            .bg(rgba(0x80808033u32))
                            .child(
                                div()
                                    .h(px(6.0))
                                    .w(gpui::relative(pct as f32 / 100.0))
                                    .rounded(px(3.0))
                                    .bg(rgba(0x4DA6FFFFu32)),
                            ),
                    )
                    .child(
                        div().flex().flex_row().justify_end().child(
                            div()
                                .px_3()
                                .py_1()
                                .rounded_md()
                                .border_1()
                                .border_color(rgba(0x80808055u32))
                                .text_size(px(12.0))
                                .cursor_pointer()
                                .child("Cancel")
                                .on_mouse_up(
                                    MouseButton::Left,
                                    move |_evt: &MouseUpEvent, _window, cx| {
                                        let app = app.clone();
                                        app.update(cx, |this, cx| this.cancel_transfer(cx));
                                    },
                                ),
                        ),
                    ),
            )
        });

        self.transfer = Some(TransferState {
            filename: filename.clone(),
            total,
            sent,
            cancel,
            _poll_task: poll_task,
        });
        cx.notify();
    }

    /// Cancel button on the progress dialog. Flips the atomic the
    /// protocol thread polls between blocks; the result channel
    /// will deliver `Cancelled` shortly and `finish_transfer` will
    /// take it from there.
    fn cancel_transfer(&mut self, _cx: &mut Context<Self>) {
        if let Some(t) = self.transfer.as_ref() {
            t.cancel.store(true, std::sync::atomic::Ordering::Relaxed);
        }
    }

    /// Tear-down. Closes the progress dialog, clears `transfer`
    /// (drops the poll task), and surfaces the outcome via toast.
    fn finish_transfer(
        &mut self,
        result: TransferResult,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let filename = self
            .transfer
            .as_ref()
            .map(|t| t.filename.clone())
            .unwrap_or_else(|| SharedString::from("file"));
        self.transfer = None;
        // Belt-and-braces: the transfer thread already cleared the
        // sink, but if it died on a panic before reaching that line
        // we want to make sure the read path still recovers.
        if let Some(io) = self.transfer_io.as_ref() {
            *io.transfer_sink.lock().unwrap() = None;
        }
        window.close_dialog(cx);
        match result {
            TransferResult::Ok => {
                window.push_notification(
                    Notification::success(SharedString::from(format!(
                        "Sent \u{201C}{filename}\u{201D}"
                    ))),
                    cx,
                );
            }
            TransferResult::Err(msg) => {
                window.push_notification(
                    Notification::error(SharedString::from(format!(
                        "Transfer failed: {msg}"
                    ))),
                    cx,
                );
            }
        }
        cx.notify();
    }

    /// Choose… button on the Send file dialog. Opens the OS picker;
    /// once a path comes back we stash it on the dialog's shared
    /// path mutex and notify so the dialog re-renders with the
    /// filename + the Send button enabled.
    fn send_file_choose(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Choose a file to send".into()),
        });
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(paths))) = receiver.await else { return };
            let Some(path) = paths.into_iter().next() else { return };
            let _ = this.update(cx, |this, cx| {
                if let Some(state) = this.send_file.as_ref() {
                    *state.selected_path.lock().unwrap() = Some(path);
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Send button. Reads the protocol pick + path off the dialog
    /// state, clears the dialog state, then defers to the existing
    /// `kick_off_transfer` to do the actual byte pumping. Closing
    /// the picker dialog is done inside `kick_off_transfer` (it
    /// reuses the same dialog slot for the progress UI).
    fn send_file_confirm(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(state) = self.send_file.take() else { return };
        let path = state.selected_path.lock().unwrap().clone();
        let Some(path) = path else {
            // No file picked yet — restore state instead of leaving
            // the dialog wired to a torn-down handle.
            self.send_file = Some(state);
            return;
        };
        let id = read_select(&state.protocol, cx);
        let variant = match id.as_str() {
            "xmodem-crc" => Some(XModemVariant::Crc),
            "xmodem-1k" => Some(XModemVariant::OneKilo),
            "xmodem-classic" => Some(XModemVariant::Classic),
            // Anything else (including the YMODEM id) → no variant,
            // which `kick_off_transfer` reads as "use YMODEM".
            _ => None,
        };
        self.kick_off_transfer(path, variant, window, cx);
    }

    /// Cancel / X / overlay-click. Drops dialog state and asks the
    /// dialog layer to close. Safe to call when the dialog is
    /// already closed (close_dialog is a no-op then).
    fn close_send_file_dialog(&mut self, cx: &mut Context<Self>) {
        if self.send_file.take().is_some() {
            cx.notify();
        }
    }

    /// Open a new top-level Baudrun window with a fresh `AppView`.
    /// Stores + `SettingsBus` are shared with this window's instance
    /// so settings stay in sync across the two; the new window
    /// starts disconnected with a blank terminal.
    fn open_new_window(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let scrollback = self.settings_bus.read(cx).current().effective_scrollback();
        let new_terminal = cx.new(|cx| {
            TerminalView::new(24, 80, Palette::baudrun(), scrollback, cx)
        });
        if let Err(err) = open_app_window(
            cx,
            WindowInit::Fresh(new_terminal),
            self.profile_store.clone(),
            self.settings_bus.clone(),
            self.skins_store.clone(),
            self.highlight_store.clone(),
            self.themes_store.clone(),
        ) {
            log::error!("open new window: {err}");
        }
    }

    /// Open a fresh window and immediately connect it to the named
    /// profile. Used by the right-click context menu on profile rows.
    pub(crate) fn connect_profile_in_new_window(
        &mut self,
        profile_id: String,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let scrollback = self.settings_bus.read(cx).current().effective_scrollback();
        let new_terminal = cx.new(|cx| {
            TerminalView::new(24, 80, Palette::baudrun(), scrollback, cx)
        });
        if let Err(err) = open_app_window(
            cx,
            WindowInit::FreshAutoConnect {
                terminal: new_terminal,
                profile_id,
            },
            self.profile_store.clone(),
            self.settings_bus.clone(),
            self.skins_store.clone(),
            self.highlight_store.clone(),
            self.themes_store.clone(),
        ) {
            log::error!("connect in new window: {err}");
        }
    }

    /// Open the right-click context menu anchored at the click
    /// position. Called by the sidebar profile rows on right-mouse-
    /// down. `pos` is in window-relative coordinates so the popup
    /// lands under the cursor regardless of where the row sat.
    fn open_profile_context_menu(
        &mut self,
        profile_id: String,
        pos: gpui::Point<gpui::Pixels>,
        cx: &mut Context<Self>,
    ) {
        self.profile_context_menu = Some(ProfileContextMenu {
            profile_id,
            pos,
        });
        cx.notify();
    }

    /// Dismiss the right-click context menu. Idempotent — safe to
    /// fire from the global mouse-up listener every time.
    fn dismiss_profile_context_menu(&mut self, cx: &mut Context<Self>) {
        if self.profile_context_menu.take().is_some() {
            cx.notify();
        }
    }

    /// Take ownership of every piece of live-session state from this
    /// AppView and return it as a [`SessionBundle`]. Returns `None`
    /// when the window isn't connected to a profile (nothing to
    /// move). The fields cleared here mirror the destination's
    /// `install_session` write set so a round trip is lossless.
    fn extract_session(&mut self) -> Option<SessionBundle> {
        let connected_profile_id = self.connected_profile_id.take()?;
        let serial_disconnect = self.serial_disconnect.take()?;
        let transfer_io = self.transfer_io.take()?;
        Some(SessionBundle {
            terminal: self.terminal.clone(),
            drain_task: self.drain_task.take(),
            serial_disconnect,
            transfer_io,
            transfer: self.transfer.take(),
            connected_profile_id,
            auto_reconnect_for: self.auto_reconnect_for.take(),
            auto_reconnect_task: self.auto_reconnect_task.take(),
            last_highlight_sig: self.last_highlight_sig.take(),
        })
    }

    /// Install a moved [`SessionBundle`] onto this AppView. Replaces
    /// the placeholder TerminalView with the moved one, takes over
    /// the OS-thread token, drain task, and transfer state, and
    /// re-applies the palette so chrome that depends on the
    /// connected profile (status dot, header, status bar text)
    /// reflects the moved session immediately.
    pub fn install_session(
        &mut self,
        bundle: SessionBundle,
        cx: &mut Context<Self>,
    ) {
        self.terminal = bundle.terminal;
        self.drain_task = bundle.drain_task;
        self.serial_disconnect = Some(bundle.serial_disconnect);
        self.transfer_io = Some(bundle.transfer_io);
        self.transfer = bundle.transfer;
        self.connected_profile_id = Some(bundle.connected_profile_id);
        self.auto_reconnect_for = bundle.auto_reconnect_for;
        self.auto_reconnect_task = bundle.auto_reconnect_task;
        self.last_highlight_sig = bundle.last_highlight_sig;
        // A migrated session always lands "live" — the destination
        // user expects to see the terminal viewport in the new
        // window, not a suspended placeholder. Source's `suspended`
        // flag doesn't follow because `extract_session` doesn't
        // bundle it.
        self.suspended = false;
        // Re-resolve the palette now that `connected_profile_id` is
        // set on this AppView — covers the case where the moved
        // session's profile carried a per-profile theme override.
        self.apply_palette(cx);
        cx.notify();
    }

    /// Suspend the live session. The port stays open and bytes keep
    /// flowing into the TerminalView's scrollback in the background;
    /// the right pane swaps from the terminal viewport to the
    /// editor for the connected profile, so the user can browse
    /// other UI without losing their session.
    pub(crate) fn suspend_session(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Only meaningful when actually connected. Reconnecting
        // counts: the user might want to suspend during a flap.
        if self.connected_profile_id.is_none()
            && self.auto_reconnect_for.is_none()
        {
            return;
        }
        if self.suspended {
            return;
        }
        self.suspended = true;
        // Auto-open the connected profile's editor so the right
        // pane has something visible. Without this the suspended
        // window would fall back to the welcome screen, which the
        // user could mistake for "disconnected".
        if self.editor.is_none() {
            if let Some(id) = self
                .connected_profile_id
                .clone()
                .or_else(|| self.auto_reconnect_for.clone())
            {
                self.open_editor_for(id, window, cx);
            }
        }
        cx.notify();
    }

    /// Resume a suspended session. Closes any open editor (matching
    /// Tauri's "Resume snaps you back to terminal" UX) and re-grabs
    /// focus for the viewport so typing lands in the grid without
    /// an extra click.
    pub(crate) fn resume_session(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.suspended {
            return;
        }
        self.suspended = false;
        self.editor = None;
        let viewport_focus = self.terminal.read(cx).focus_handle().clone();
        viewport_focus.focus(window, cx);
        cx.notify();
    }

    // ----- Shortcut-action handlers ---------------------------------
    // One method per `settings_view::SHORTCUT_ACTIONS` id that
    // doesn't already have a 1:1 existing entry point. The menubar
    // (`main::install_app_menu`) registers a global gpui Action +
    // KeyBinding for each; AppView::render dispatches each Action
    // to the matching method below via `.on_action(cx.listener(...))`.
    //
    // These mirror the on-screen affordances (header buttons, form
    // Connect button, sidebar New Profile icon) — pressing the
    // shortcut should behave identically to clicking the equivalent
    // UI control. Methods are no-ops when the corresponding control
    // would be disabled (e.g. Disconnect when nothing is connected)
    // so a stray Cmd-Shift-D doesn't fire a hidden side effect.

    /// Connect via shortcut. Mirrors the form's Connect button when
    /// an editor is open (save-then-connect) and the sidebar row's
    /// double-click when only the selection is set. No-op when
    /// there's neither — keeps a stray keystroke from doing
    /// something surprising in the empty-state.
    pub(crate) fn shortcut_connect(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.editor.is_some() {
            self.save_and_connect(window, cx);
            return;
        }
        let Some(id) = self.selected_profile_id.clone() else {
            return;
        };
        // Don't re-connect to the already-live profile — would tear
        // down the current session for no reason.
        if self.connected_profile_id.as_deref() == Some(id.as_str()) {
            return;
        }
        let Some(profile) = self.profile_store.get(&id) else {
            return;
        };
        self.connect_to(profile, cx);
    }

    /// Clear the terminal viewport. Forwards to the TerminalView
    /// entity; same code path the header's Clear button takes.
    pub(crate) fn shortcut_clear_terminal(&mut self, cx: &mut Context<Self>) {
        let terminal = self.terminal.clone();
        terminal.update(cx, |t, cx| t.clear_screen(cx));
    }

    /// Open the form for a fresh profile. Same entry point as the
    /// sidebar's "+" icon (`open_editor`).
    pub(crate) fn shortcut_new_profile(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_editor(window, cx);
    }

    /// Show the standard macOS-style "About Baudrun" sheet. Opens
    /// a gpui-component Dialog over the active window with name,
    /// version, copyright, and a clickable GitHub link. Triggered
    /// by the Baudrun → About menu item.
    /// Whether this window has a serial session whose teardown
    /// the user would notice — a live connection, an in-flight
    /// X/YMODEM transfer, or an auto-reconnect retry that's
    /// actively polling the OS for the port to come back. Used
    /// by the Quit-confirmation prompt; false for windows that
    /// are sitting on the welcome screen or staring at a closed
    /// editor.
    pub(crate) fn has_live_session(&self) -> bool {
        self.connected_profile_id.is_some()
            || self.transfer.is_some()
            || self.auto_reconnect_for.is_some()
    }

    pub(crate) fn shortcut_about(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        window.open_dialog(cx, |dialog, _window, _cx| {
            // No accelerator on this one — About is menu-only,
            // hence no Settings → Shortcuts entry. The title bar's
            // close button (`.close_button(true)`) is enough to
            // dismiss; no OK / Cancel footer needed for a sheet
            // that's purely informational.
            dialog
                .title("About Baudrun")
                .close_button(true)
                .width(px(360.0))
                .child(about_dialog_body())
        });
    }

    /// Open the currently-selected profile in a new window, already
    /// connected. Falls back to `open_new_window` (empty welcome
    /// screen) when no profile is selected — mirrors the right-
    /// click context menu's "Open in new window" but keyed by the
    /// sidebar selection rather than the row under the cursor.
    pub(crate) fn shortcut_open_in_new_window(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match self.selected_profile_id.clone() {
            Some(id) => self.connect_profile_in_new_window(id, window, cx),
            None => self.open_new_window(window, cx),
        }
    }

    /// Thin wrappers around `shortcut_bump_font` so main.rs's
    /// App-level handlers don't need to import the private
    /// `FontBump` enum.
    pub(crate) fn shortcut_bump_font_increase(&mut self, cx: &mut Context<Self>) {
        self.shortcut_bump_font(FontBump::Increase, cx);
    }
    pub(crate) fn shortcut_bump_font_decrease(&mut self, cx: &mut Context<Self>) {
        self.shortcut_bump_font(FontBump::Decrease, cx);
    }
    pub(crate) fn shortcut_bump_font_reset(&mut self, cx: &mut Context<Self>) {
        self.shortcut_bump_font(FontBump::Reset, cx);
    }

    /// Bump the persisted font size by the requested delta (clamped
    /// to a readable range) and persist via SettingsBus. The
    /// `Updated` event fires our own subscription which re-applies
    /// the size to the terminal in `apply_settings` — so we don't
    /// need to touch the TerminalView directly here.
    fn shortcut_bump_font(&mut self, op: FontBump, cx: &mut Context<Self>) {
        let mut next = self.settings_bus.read(cx).current().clone();
        let current = if next.font_size > 0 {
            next.font_size
        } else {
            crate::terminal_grid::FONT_SIZE_PX as i32
        };
        let target = match op {
            FontBump::Increase => (current + 1).min(48),
            FontBump::Decrease => (current - 1).max(8),
            FontBump::Reset => crate::terminal_grid::FONT_SIZE_PX as i32,
        };
        if target == current {
            return;
        }
        next.font_size = target;
        if let Err(err) =
            self.settings_bus.update(cx, |bus, cx| bus.replace(next, cx))
        {
            log::error!("shortcut: font bump persist failed: {err}");
        }
    }

    /// Move the live serial session out of this window into a freshly
    /// opened one. The source window swaps its terminal for a blank
    /// placeholder and ends up on the welcome screen; the destination
    /// window comes up already connected with the same terminal
    /// contents (scrollback included), the same OS thread, and any
    /// in-flight file transfer intact.
    fn move_session_to_new_window(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(bundle) = self.extract_session() else { return };
        // Replace this window's terminal with a blank one so the
        // source view doesn't render someone else's connected grid.
        let scrollback = self.settings_bus.read(cx).current().effective_scrollback();
        self.terminal = cx.new(|cx| {
            TerminalView::new(24, 80, Palette::baudrun(), scrollback, cx)
        });
        cx.notify();

        if let Err(err) = open_app_window(
            cx,
            WindowInit::WithSession(bundle),
            self.profile_store.clone(),
            self.settings_bus.clone(),
            self.skins_store.clone(),
            self.highlight_store.clone(),
            self.themes_store.clone(),
        ) {
            log::error!("move session to new window: {err}");
        }
    }
}

/// `⋯` button + drop-down menu rendered inline in the session
/// header. The container is `relative` so the menu (positioned
/// `absolute` below) anchors to it; `deferred` puts the panel above
/// other toolbar siblings in paint order.
fn session_overflow_button(
    s: SkinTokens,
    open: bool,
    cx: &mut Context<AppView>,
) -> gpui::Div {
    let panel: Option<gpui::AnyElement> = if open {
        Some(
            deferred(
                div()
                    .absolute()
                    .top_full()
                    .right_0()
                    .mt_1()
                    .min_w(px(180.0))
                    .bg(rgba(s.bg_panel))
                    .border_1()
                    .border_color(rgba(s.border_subtle))
                    .rounded(px(s.radius_md))
                    .shadow_md()
                    .py_1()
                    .child(profile_menu_item(
                        s,
                        "Send Break",
                        cx.listener(|this, _: &MouseUpEvent, window, cx| {
                            this.send_break_now(window, cx);
                        }),
                    ))
                    .child(profile_menu_item(
                        s,
                        "Send Hex\u{2026}",
                        cx.listener(|this, _: &MouseUpEvent, window, cx| {
                            this.open_send_hex(window, cx);
                        }),
                    ))
                    .child(profile_menu_item(
                        s,
                        "Send File\u{2026}",
                        cx.listener(|this, _: &MouseUpEvent, window, cx| {
                            this.start_send_file(window, cx);
                        }),
                    ))
                    .child(profile_menu_item(
                        s,
                        "Move to New Window",
                        cx.listener(|this, _: &MouseUpEvent, window, cx| {
                            this.move_session_to_new_window(window, cx);
                        }),
                    )),
            )
            .into_any_element(),
        )
    } else {
        None
    };
    div()
        .relative()
        .child(
            div()
                .id("session-overflow-btn")
                .px_3()
                .py_1()
                .rounded_md()
                .border_1()
                .border_color(rgba(s.border_subtle))
                .bg(rgba(s.bg_input))
                .text_color(rgba(s.fg_primary))
                .text_size(px(13.0))
                .cursor_pointer()
                .hover(move |st| st.bg(rgba(s.bg_hover)))
                .tooltip(|window, cx| {
                    Tooltip::new(SharedString::from("More actions"))
                        .build(window, cx)
                })
                .child("\u{22EF}")
                .on_mouse_up(
                    MouseButton::Left,
                    cx.listener(|this, _: &MouseUpEvent, _, cx| {
                        // Without `stop_propagation`, the AppView
                        // root's mouse-up listener (which dismisses
                        // any open popup) fires immediately after
                        // and closes the menu we just opened.
                        cx.stop_propagation();
                        this.toggle_session_overflow(cx);
                    }),
                ),
        )
        .children(panel)
}

/// Parse a Send Hex string into raw bytes. Accepts space-separated,
/// comma-separated, compact, and `0x`-prefixed input — same rules
/// as the Tauri version's `parseHex`. Returns `Err(reason)` for
/// odd lengths, non-hex characters, or empty input so the caller
/// can surface the reason inline.
fn parse_hex_string(raw: &str) -> Result<Vec<u8>, &'static str> {
    let cleaned: String = raw
        .replace("0x", "")
        .replace("0X", "")
        .chars()
        .filter(|c| !c.is_whitespace() && *c != ',')
        .collect();
    if cleaned.is_empty() {
        return Ok(Vec::new());
    }
    if cleaned.len() % 2 != 0 {
        return Err("odd number of hex digits");
    }
    if !cleaned.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err("non-hex characters");
    }
    let mut out = Vec::with_capacity(cleaned.len() / 2);
    let bytes = cleaned.as_bytes();
    for chunk in bytes.chunks_exact(2) {
        let s = std::str::from_utf8(chunk).unwrap();
        out.push(u8::from_str_radix(s, 16).map_err(|_| "non-hex characters")?);
    }
    Ok(out)
}

/// Stable id used as the YMODEM Select option (the default pick).
const YMODEM_PROTOCOL_ID: &str = "ymodem";

/// Option list for the Send file dialog's Protocol select. Order +
/// labels mirror the Tauri dialog so the muscle memory transfers.
fn transfer_protocol_opts() -> Vec<Opt> {
    vec![
        Opt::new(
            YMODEM_PROTOCOL_ID,
            "YMODEM \u{2014} 1024-byte blocks with filename + size",
        ),
        Opt::new(
            "xmodem-crc",
            "XMODEM-CRC \u{2014} 128-byte blocks with CRC-16",
        ),
        Opt::new(
            "xmodem-1k",
            "XMODEM-1K \u{2014} 1024-byte blocks with CRC-16",
        ),
        Opt::new(
            "xmodem-classic",
            "XMODEM-Classic \u{2014} 128-byte blocks with checksum",
        ),
    ]
}

/// Small uppercase-ish field label above an input row.
fn send_file_field_label(label: &'static str) -> gpui::Div {
    div()
        .text_size(px(11.0))
        .text_color(rgba(0x808080CCu32))
        .child(label)
}

/// Read-only path display — input-styled pill that shows the chosen
/// filename (or a muted "No file selected" placeholder).
fn send_file_path_pill(label: String) -> gpui::Div {
    let placeholder = label.is_empty();
    let text: SharedString = if placeholder {
        SharedString::from("No file selected")
    } else {
        SharedString::from(label)
    };
    div()
        .flex_1()
        .px_3()
        .py(px(6.0))
        .rounded_md()
        .border_1()
        .border_color(rgba(0x80808055u32))
        .bg(rgba(0x80808014u32))
        .text_size(px(13.0))
        .text_color(rgba(if placeholder {
            0x808080AAu32
        } else {
            0xE4E4E7FFu32
        }))
        .child(text)
}

/// "Choose…" — secondary pill button next to the path display.
fn send_file_choose_button<F>(on_click: F) -> gpui::Div
where
    F: Fn(&MouseUpEvent, &mut Window, &mut gpui::App) + 'static,
{
    div()
        .px_3()
        .py(px(6.0))
        .rounded_md()
        .border_1()
        .border_color(rgba(0x80808055u32))
        .bg(rgba(0x80808022u32))
        .text_size(px(13.0))
        .cursor_pointer()
        .hover(|st| st.bg(rgba(0x4DA6FF22u32)))
        .child("Choose\u{2026}")
        .on_mouse_up(MouseButton::Left, on_click)
}

/// Cancel-style button at the bottom of the dialog. Quiet styling
/// (matches Tauri's secondary button) so it doesn't compete with
/// the primary Send button.
fn send_file_secondary_button<F>(label: &'static str, on_click: F) -> gpui::Div
where
    F: Fn(&MouseUpEvent, &mut Window, &mut gpui::App) + 'static,
{
    div()
        .px_4()
        .py(px(6.0))
        .rounded_md()
        .border_1()
        .border_color(rgba(0x80808055u32))
        .text_size(px(13.0))
        .cursor_pointer()
        .hover(|st| st.bg(rgba(0x80808033u32)))
        .child(label)
        .on_mouse_up(MouseButton::Left, on_click)
}

/// Send-style primary button. Disabled state (when `enabled` is
/// false) still renders but ignores clicks and dims out, matching
/// Tauri's "you must pick a file first" affordance.
fn send_file_primary_button<F>(label: &'static str, enabled: bool, on_click: F) -> gpui::Div
where
    F: Fn(&MouseUpEvent, &mut Window, &mut gpui::App) + 'static,
{
    let text_color = if enabled { 0xFFFFFFFFu32 } else { 0xFFFFFFAAu32 };
    let bg = if enabled { 0x4DA6FFFFu32 } else { 0x4DA6FF66u32 };
    let mut btn = div()
        .px_4()
        .py(px(6.0))
        .rounded_md()
        .bg(rgba(bg))
        .text_size(px(13.0))
        .text_color(rgba(text_color))
        .child(label);
    if enabled {
        btn = btn.cursor_pointer().on_mouse_up(MouseButton::Left, on_click);
    }
    btn
}

/// Bundle of live serial-session state that can be moved from one
/// window's `AppView` to another. Captures everything the source
/// AppView held about the live connection — the TerminalView entity
/// (bytes already on screen and in scrollback), the OS-side
/// disconnect token, the read-loop drain task, transfer I/O, and
/// any in-flight transfer + auto-reconnect bookkeeping. Built by
/// [`AppView::extract_session`] on the source side and consumed by
/// [`AppView::install_session`] on the destination side.
pub struct SessionBundle {
    terminal: Entity<TerminalView>,
    drain_task: Option<Task<()>>,
    serial_disconnect: serial_io::Disconnect,
    transfer_io: TransferIo,
    transfer: Option<TransferState>,
    connected_profile_id: String,
    auto_reconnect_for: Option<String>,
    auto_reconnect_task: Option<Task<()>>,
    last_highlight_sig: Option<Vec<(String, String, bool)>>,
}

/// What kind of window to open via [`open_app_window`]. `Fresh`
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
    WithSession(SessionBundle),
    FreshAutoConnect {
        terminal: Entity<TerminalView>,
        profile_id: String,
    },
}

/// Open a new top-level Baudrun window with a fresh `AppView`. The
/// stores + `SettingsBus` are shared (cloned `Rc`/`Entity`) so each
/// window's settings live-update in lockstep with the others, but
/// the `TerminalView`, sidebar, profile editor, and transfer state
/// are per-window — connecting in one window doesn't touch the
/// terminal in another. Used both at startup (one window) and from
/// `AppView::open_new_window` / `move_session_to_new_window`.
/// Convert a saved geometry record into the bounds shape
/// `WindowOptions` wants. Returns `None` when the saved record is
/// missing dimensions — caller falls back to the centered default.
fn geometry_to_bounds(g: &settings::WindowGeometry) -> Option<Bounds<Pixels>> {
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
fn bounds_to_geometry(b: Bounds<Pixels>) -> settings::WindowGeometry {
    settings::WindowGeometry {
        x: f32::from(b.origin.x) as i32,
        y: f32::from(b.origin.y) as i32,
        width: f32::from(b.size.width) as i32,
        height: f32::from(b.size.height) as i32,
    }
}

pub fn open_app_window(
    cx: &mut gpui::App,
    init: WindowInit,
    profile_store: Rc<profiles::Store>,
    settings_bus: Entity<SettingsBus>,
    skins_store: Rc<skins::Store>,
    highlight_store: Rc<highlight::Store>,
    themes_store: Rc<themes::Store>,
) -> gpui::Result<WindowHandle<Root>> {
    let current_settings = settings_bus.read(cx).current().clone();
    let restore_state = !current_settings.disable_window_state_restore;
    let bounds = restore_state
        .then(|| current_settings.main_window.as_ref().and_then(geometry_to_bounds))
        .flatten()
        .unwrap_or_else(|| {
            Bounds::centered(None, gpui::size(px(1100.0), px(720.0)), cx)
        });
    let settings_bus_for_close = settings_bus.clone();
    cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(TitlebarOptions {
                title: Some("Baudrun".into()),
                ..Default::default()
            }),
            ..Default::default()
        },
        move |window, cx| {
            // Snapshot bounds when the OS asks the window to close.
            // Reads the live `disable_window_state_restore` flag so a
            // user who turned the feature off after open doesn't get
            // their pin overwritten on quit.
            window.on_window_should_close(cx, move |window, cx| {
                let geom = bounds_to_geometry(window.bounds());
                settings_bus_for_close.update(cx, |bus, cx| {
                    let mut next = bus.current().clone();
                    if !next.disable_window_state_restore {
                        next.main_window = Some(geom);
                        if let Err(err) = bus.replace(next, cx) {
                            log::error!("save window state: {err}");
                        }
                    }
                });
                true
            });
            // Snapshot the OS appearance for the picker's "Auto" pick;
            // the appearance observer below picks up live changes.
            let system_dark = matches!(
                window.appearance(),
                gpui::WindowAppearance::Dark | gpui::WindowAppearance::VibrantDark
            );
            let (terminal, session, auto_connect_id) = match init {
                WindowInit::Fresh(t) => (t, None, None),
                WindowInit::WithSession(bundle) => {
                    (bundle.terminal.clone(), Some(bundle), None)
                }
                WindowInit::FreshAutoConnect {
                    terminal,
                    profile_id,
                } => (terminal, None, Some(profile_id)),
            };
            let app_view = cx.new(|cx| {
                AppView::new(
                    terminal,
                    profile_store,
                    settings_bus,
                    skins_store,
                    highlight_store,
                    themes_store,
                    system_dark,
                    cx,
                )
            });
            app_view.update(cx, |this, view_cx| {
                this.attach_appearance_observer(window, view_cx);
                if let Some(bundle) = session {
                    this.install_session(bundle, view_cx);
                }
                if let Some(id) = auto_connect_id {
                    if let Some(profile) = this.profile_store.get(&id) {
                        this.connect_to(profile, view_cx);
                    } else {
                        log::warn!(
                            "auto-connect: profile {id:?} not found in store"
                        );
                    }
                }
            });
            cx.new(|cx| Root::new(app_view, window, cx))
        },
    )
}

impl Render for AppView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let s = *cx.global::<SkinTokens>();
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
            Some(editor) => {
                let render = EditorRender::from(editor, cx);
                let packs = self.highlight_store.list();
                let global_enabled = self
                    .settings_bus
                    .read(cx)
                    .current()
                    .enabled_highlight_presets
                    .clone()
                    .unwrap_or_else(|| {
                        settings::Settings::default()
                            .enabled_highlight_presets
                            .unwrap_or_default()
                    });
                // Resume banner only renders when the editor on
                // screen IS for the connected profile and we're
                // suspended — clicking Resume goes back to ITS
                // terminal viewport.
                let show_resume = self.suspended
                    && editor.profile_id.is_some()
                    && editor.profile_id.as_deref()
                        == self.connected_profile_id.as_deref();
                let form = form_pane(render, packs, global_enabled, show_resume, cx);
                if show_resume {
                    // Wrap so the banner stacks above the form.
                    // `min_w_0` mirrors form_pane's own shrink
                    // setting — without it the wrapper holds the
                    // form to its intrinsic width and long card
                    // descriptions push the right-edge columns off
                    // the visible window.
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_w_0()
                        .min_h_0()
                        .child(suspended_banner(s, cx))
                        .child(form)
                        .into_any_element()
                } else {
                    form.into_any_element()
                }
            }
            None if self.suspended && connected_profile.is_some() => {
                // Suspended with no editor open — render the
                // placeholder so the terminal viewport stays
                // hidden until the user explicitly resumes.
                suspended_pane(
                    s,
                    connected_profile.clone().expect("checked is_some"),
                    cx,
                )
                .into_any_element()
            }
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
                        .child(session_header(
                            profile,
                            reconnecting,
                            self.session_overflow_open,
                            cx,
                        ))
                        .child(div().flex_1().min_h_0().child(terminal))
                        .into_any_element()
                }
                None => welcome_pane(s, has_profiles).into_any_element(),
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
            // Opaque shell base. Skins layer their translucent
            // panels (`bg_main`, `bg_panel`, `bg_sidebar`) on top
            // of this — without it the macOS-26 / Baudrun skins
            // look uniformly dark because their `--bg-main` is
            // translucent white with nothing solid beneath.
            .bg(rgba(s.bg_window))
            // (Shortcut actions are handled at the App level —
            // `main::install_app_menu` registers global on_action
            // listeners that route via `dispatch_to_app_view` into
            // the active window's AppView. Per-window `.on_action`
            // handlers on this div would also fire — global
            // capture and window capture both dispatch the same
            // action — and we'd hit every action twice per
            // keystroke. The global path covers both menu clicks
            // (where focus isn't in the AppView tree) and
            // keystrokes (where it is) by deferring into the
            // active window's AppView entity directly.)
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
                            // No bg here — `bg_window` (painted on
                            // the outermost div) is the opaque
                            // base, and the sidebar / right pane
                            // each paint their own translucent
                            // layer (`bg_sidebar`, `bg_main`,
                            // `bg_panel`) directly on top. Painting
                            // an extra `bg_main` here would stack
                            // alpha and brighten everything inside
                            // beyond what the skin specifies.
            // -- sidebar --
            .child(
                div()
                    .w(px(SIDEBAR_WIDTH_PX))
                    .h_full()
                    .bg(rgba(s.bg_sidebar))
                    .border_r_1()
                    .border_color(rgba(s.border_subtle))
                    .px_2()
                    .py_3()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .text_color(rgba(s.fg_primary))
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
                        s,
                        connected_for_status.as_ref(),
                        reconnecting,
                        editor_name.as_deref(),
                        Some(self.terminal.read(cx).scrollback_state()),
                    )),
            )
            // -- gpui-component overlay layers (dialogs, toasts) --
            .children(dialog_layer)
            .children(notification_layer)
            // -- profile-row right-click context menu --
            .children(profile_context_menu_overlay(self, cx))
            // Any mouse-up anywhere dismisses open popup menus.
            // Menu items run their own `on_mouse_up` first (because
            // gpui dispatches inside-out); this is the catch-all
            // for clicks outside the menu panels.
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _: &MouseUpEvent, _, cx| {
                    this.dismiss_profile_context_menu(cx);
                    this.dismiss_session_overflow(cx);
                }),
            )
    }
}

/// Build the deferred + anchored overlay that renders the profile-
/// row right-click context menu. Returns `None` when no menu is
/// open. Lives outside `Render::render` to keep the body readable.
fn profile_context_menu_overlay(
    app: &AppView,
    cx: &mut Context<AppView>,
) -> Option<gpui::AnyElement> {
    let menu = app.profile_context_menu.as_ref()?;
    let s = *cx.global::<SkinTokens>();
    let profile_id = menu.profile_id.clone();
    let pos = menu.pos;
    // When the right-clicked profile is the one this window is
    // already connected to, "Connect in New Window" would either
    // race the existing session for the same port or quietly steal
    // it. Surface "Move to New Window" instead, reusing the same
    // detach/install_session machinery the toolbar Detach button
    // already drives.
    let is_connected_row =
        app.connected_profile_id.as_deref() == Some(profile_id.as_str());

    let item: gpui::Div = if is_connected_row {
        profile_menu_item(
            s,
            "Move Session to New Window",
            cx.listener(move |this, _: &MouseUpEvent, window, cx| {
                this.profile_context_menu = None;
                this.move_session_to_new_window(window, cx);
            }),
        )
    } else {
        profile_menu_item(
            s,
            "Connect in New Window",
            cx.listener(move |this, _: &MouseUpEvent, window, cx| {
                let id = profile_id.clone();
                this.profile_context_menu = None;
                this.connect_profile_in_new_window(id, window, cx);
            }),
        )
    };

    let panel = div()
        .min_w(px(220.0))
        .bg(rgba(s.bg_panel))
        .border_1()
        .border_color(rgba(s.border_subtle))
        .rounded(px(s.radius_md))
        .shadow_md()
        .py_1()
        .child(item);
    Some(deferred(anchored().position(pos).child(panel)).into_any_element())
}

/// One row inside the profile right-click menu. Plain hover-styled
/// div — keeping it hand-rolled rather than reaching for
/// gpui-component's PopupMenu (which routes through the Action
/// system) since we have a single click handler that already needs
/// the per-row profile id baked in.
fn profile_menu_item<F>(
    s: SkinTokens,
    label: &'static str,
    on_click: F,
) -> gpui::Div
where
    F: Fn(&MouseUpEvent, &mut Window, &mut gpui::App) + 'static,
{
    let hover_bg = s.bg_hover;
    div()
        .px_3()
        .py(px(6.0))
        .text_size(px(13.0))
        .text_color(rgba(s.fg_primary))
        .cursor_pointer()
        .hover(move |st| st.bg(rgba(hover_bg)))
        .child(label)
        .on_mouse_up(MouseButton::Left, on_click)
}

/// Idle splash screen — shown when the app is launched with no
/// connected profile and the user hasn't opened the editor yet.
/// Mirrors the Tauri version's "no terminal until you pick a
/// profile" default. Wording adapts to whether any profiles
/// exist: with profiles, prompt to pick one; without, prompt to
/// click the `+` to create one.
/// Thin banner at the top of the editor when the connected profile
/// is being viewed while suspended. Click Resume to switch back to
/// the live terminal viewport.
fn suspended_banner(
    s: SkinTokens,
    cx: &mut Context<AppView>,
) -> impl IntoElement {
    div()
        .w_full()
        .px_4()
        .py_2()
        .bg(rgba(s.bg_active))
        .border_b_1()
        .border_color(rgba(s.border_subtle))
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .child(
            div()
                .flex()
                .flex_col()
                .child(
                    div()
                        .text_size(px(12.0))
                        .text_color(rgba(s.fg_primary))
                        .child("Session suspended"),
                )
                .child(
                    div()
                        .text_size(px(11.0))
                        .text_color(rgba(s.fg_secondary))
                        .child(
                            "Port still open. Bytes keep flowing into scrollback.",
                        ),
                ),
        )
        .child(primary_button(s, "Resume").on_mouse_up(
            MouseButton::Left,
            cx.listener(|this, _: &MouseUpEvent, window, cx| {
                this.resume_session(window, cx);
            }),
        ))
}

/// Right-pane placeholder shown when the session is suspended and
/// no editor is open. Shows the connected profile's name + port and
/// a Resume button so the user has a one-click way back to the
/// live terminal.
fn suspended_pane(
    s: SkinTokens,
    profile: Profile,
    cx: &mut Context<AppView>,
) -> impl IntoElement {
    let port_line = if profile.port_name.is_empty() {
        "(no port)".to_string()
    } else {
        format!("{} @ {}", profile.port_name, profile.baud_rate)
    };
    div()
        .flex_1()
        .h_full()
        .bg(rgba(s.bg_main))
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap_3()
        .child(
            div()
                .text_size(px(11.0))
                .text_color(rgba(s.fg_tertiary))
                .child("SESSION SUSPENDED"),
        )
        .child(
            div()
                .text_size(px(20.0))
                .text_color(rgba(s.fg_primary))
                .child(profile.name.clone()),
        )
        .child(
            div()
                .text_size(px(12.0))
                .text_color(rgba(s.fg_secondary))
                .child(port_line),
        )
        .child(
            div()
                .text_size(px(12.0))
                .text_color(rgba(s.fg_tertiary))
                .child("Port stays open; bytes keep flowing into scrollback."),
        )
        .child(primary_button(s, "Resume").on_mouse_up(
            MouseButton::Left,
            cx.listener(|this, _: &MouseUpEvent, window, cx| {
                this.resume_session(window, cx);
            }),
        ))
}

/// Body content for the About dialog: app name, version, one-line
/// description, copyright, and a GitHub link. The Dialog wrapper
/// supplies the title bar and Close button; we just hand back the
/// flex column that sits below the title.
fn about_dialog_body() -> impl IntoElement {
    const GITHUB_URL: &str = "https://github.com/packetThrower/Baudrun";
    let version = env!("CARGO_PKG_VERSION");
    div()
        .flex()
        .flex_col()
        .gap_3()
        .pt_2()
        .child(
            div()
                .text_xl()
                .font_weight(gpui::FontWeight::BOLD)
                .child("Baudrun"),
        )
        .child(
            div()
                .text_sm()
                .opacity(0.75)
                .child(format!("Version {version} (prototype)")),
        )
        .child(
            // Tagline pulls from the same source-of-truth as
            // Cargo.toml's `description` so a future product-copy
            // change doesn't have to remember to update two files.
            div()
                .text_sm()
                .child(env!("CARGO_PKG_DESCRIPTION").to_string()),
        )
        .child(
            div()
                .text_sm()
                .opacity(0.65)
                .child("© 2025–2026 packetThrower / Baudrun contributors"),
        )
        .child(
            div()
                .id("about-github-link")
                .text_sm()
                .text_color(gpui::rgba(0x3b82f6ffu32))
                .cursor_pointer()
                .hover(|s| s.text_color(gpui::rgba(0x60a5faffu32)))
                .on_click(|_evt, _window, cx| cx.open_url(GITHUB_URL))
                .child("View on GitHub"),
        )
}

fn welcome_pane(s: SkinTokens, has_profiles: bool) -> impl IntoElement {
    let prompt = if has_profiles {
        "Pick a profile from the sidebar to start a session."
    } else {
        "Click the + above the profile list to create one."
    };
    div()
        .flex_1()
        .h_full()
        .bg(rgba(s.bg_main))
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap_3()
        .child(
            div()
                .text_size(px(28.0))
                .text_color(rgba(s.fg_primary))
                .child("Baudrun"),
        )
        .child(
            div()
                .text_size(px(13.0))
                .text_color(rgba(s.fg_secondary))
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
    overflow_open: bool,
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
    let s = *cx.global::<SkinTokens>();
    let dot_color = if reconnecting { s.warn } else { s.success };
    div()
        .w_full()
        .px_4()
        .py_2()
        .bg(rgba(s.bg_main))
        .border_b_1()
        .border_color(rgba(s.border_subtle))
        .text_size(px(13.0))
        .text_color(rgba(s.fg_primary))
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
                        .bg(rgba(dot_color));
                    if reconnecting && !cx.global::<crate::ReduceMotion>().0 {
                        // Match the Tauri `.dot.reconnecting`
                        // pulse: 1s ease-in-out, opacity bounces
                        // between roughly 0.35 and 1.0. gpui's
                        // `pulsating_between` returns the easing
                        // curve; the per-frame closure applies the
                        // current alpha. Skipped under prefers-
                        // reduced-motion — the orange dot's colour
                        // alone is enough signal that we're in
                        // the reconnecting state.
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
                                .text_color(rgba(s.fg_primary))
                                .child(profile.name),
                        )
                        .child(
                            div()
                                .text_size(px(11.0))
                                .text_color(rgba(s.fg_secondary))
                                .child(meta),
                        ),
                ),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .gap_2()
                .child(session_overflow_button(s, overflow_open, cx))
                .child(pill_button(s, "Clear", false).on_mouse_up(
                    MouseButton::Left,
                    cx.listener(|this, _: &MouseUpEvent, _window, cx| {
                        this.terminal.update(cx, |t, cx| t.clear_screen(cx));
                    }),
                ))
                .child(
                    div()
                        .id("session-suspend")
                        .child(pill_button(s, "Suspend", false))
                        .tooltip(|window, cx| {
                            Tooltip::new(SharedString::from(
                                "Keep session alive; return to profile",
                            ))
                            .build(window, cx)
                        })
                        .on_mouse_up(
                            MouseButton::Left,
                            cx.listener(|this, _: &MouseUpEvent, window, cx| {
                                this.suspend_session(window, cx);
                            }),
                        ),
                )
                .child(primary_button(s, "Disconnect").on_mouse_up(
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
    s: SkinTokens,
    connected: Option<&Profile>,
    reconnecting: bool,
    editing_profile_name: Option<&str>,
    scrollback: Option<(usize, usize)>,
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
    let scrollback_text = scrollback.map(|(filled, max)| format!("{filled}/{max}"));
    div()
        .w_full()
        .px_4()
        .py_1()
        .bg(rgba(s.bg_sidebar))
        .border_t_1()
        .border_color(rgba(s.border_subtle))
        .text_size(px(11.0))
        .text_color(rgba(s.fg_secondary))
        .flex()
        .flex_row()
        .items_center()
        .child(div().flex_1().child(text))
        .children(scrollback_text.map(|t| {
            div()
                .id("status-scrollback")
                .child(t)
                .tooltip(|window, cx| {
                    Tooltip::new(SharedString::from(
                        "Scrollback lines: filled / max",
                    ))
                    .build(window, cx)
                })
        }))
}

/// Sidebar header row: muted "PROFILES" label on the left, "+"
/// affordance on the right that opens the new-profile form. The
/// "+" is a div-with-click rather than a real button widget — same
/// reasoning as the rest of the sidebar (less surface area than
/// adopting `gpui_component::button` for one element).
fn sidebar_header(cx: &mut Context<AppView>) -> impl IntoElement {
    let s = *cx.global::<SkinTokens>();
    let hover_bg = s.bg_hover;
    // Shared chrome for the inline icon-buttons. Each button needs
    // its own stable id so the tooltip layer can disambiguate hover
    // targets, and a label string for the tooltip itself.
    let icon_btn = move |id: &'static str, tip: &'static str| {
        let tip_text = SharedString::from(tip);
        div()
            .id(SharedString::from(id))
            .px_2()
            .rounded_sm()
            .text_color(rgba(s.fg_primary))
            .hover(move |st| st.bg(rgba(hover_bg)))
            .cursor_pointer()
            .tooltip(move |window, cx| {
                Tooltip::new(tip_text.clone()).build(window, cx)
            })
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
                .text_color(rgba(s.fg_tertiary))
                .child("PROFILES"),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap_1()
                .child(
                    icon_btn("nav-add-profile", "New profile")
                        .text_size(px(16.0))
                        .child("+")
                        .on_mouse_up(
                            MouseButton::Left,
                            cx.listener(|this, _: &MouseUpEvent, window, cx| {
                                this.open_editor(window, cx);
                            }),
                        ),
                )
                // Unicode "two joined squares" (`⧉`) — the new-window
                // glyph. Same icon-button chrome as `+` and `⚙` so
                // the trio reads as one cluster. macOS users who
                // expect Cmd+N can still use it once Phase 8 wires
                // the application menu.
                .child(
                    icon_btn("nav-new-window", "New window")
                        .text_size(px(15.0))
                        .child("\u{29C9}")
                        .on_mouse_up(
                            MouseButton::Left,
                            cx.listener(|this, _: &MouseUpEvent, window, cx| {
                                this.open_new_window(window, cx);
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
                    icon_btn("nav-settings", "Settings")
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


// Chrome colours used to live as `const`s here, but Phase 4 slice 3
// moved them into the `SkinTokens` global so skin picks live-apply.
// Render code reads `cx.global::<SkinTokens>()`; helpers without a
// Context take the `SkinTokens` value as a parameter (Copy, 64
// bytes, cheap to pass).

/// Construct a fresh `Editor` whose every widget (text inputs +
/// selects + checkbox bool) is seeded from `profile`. Shared by
/// the new-profile path (`profile = Profile::defaults()`) and the
/// edit-profile path (`profile = store.get(id).unwrap()`) so the
/// initialisation logic for each field exists in exactly one place.
fn build_editor(
    profile_id: Option<String>,
    profile: &Profile,
    themes_store: &themes::Store,
    detect_drivers: bool,
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
    // Per-profile theme picker. Empty-id row "Use global default"
    // is the first option so the override is opt-in — saving an
    // editor with that selected reverts to inheriting from
    // settings.default_theme_id.
    let theme = {
        let mut opts = Vec::with_capacity(themes_store.list().len() + 1);
        opts.push(Opt::new("", "Use global default"));
        for t in themes_store.list() {
            let title = if t.source == "user" {
                format!("{} (custom)", t.name)
            } else {
                t.name
            };
            opts.push(Opt::new(&t.id, &title));
        }
        make_select(opts, &profile.theme_id, window, cx)
    };
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
        line_numbers: profile.line_numbers,
        log_enabled: profile.log_enabled,
        auto_reconnect: profile.auto_reconnect,
        theme,
        // `None` on the saved profile becomes "inherit global"; the
        // override flag stays false until the user explicitly opts
        // into a per-profile pack list.
        highlight: profile.highlight,
        override_highlight_packs: profile.enabled_highlight_presets.is_some(),
        enabled_highlight_packs: profile
            .enabled_highlight_presets
            .clone()
            .unwrap_or_default(),
        missing_drivers: if detect_drivers {
            detect_missing_drivers()
        } else {
            Vec::new()
        },
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
    profile.line_numbers = editor.line_numbers;
    profile.log_enabled = editor.log_enabled;
    profile.auto_reconnect = editor.auto_reconnect;
    // Empty id is the explicit "Use global default" pick — store
    // it as-is so `compute_palette` falls through to the global.
    profile.theme_id = read_select(&editor.theme, cx);
    profile.paste_warn_multiline = editor.paste_warn_multiline;
    profile.paste_slow = editor.paste_slow;
    // Empty / non-numeric → None (rolls back to the store's default
    // of 10ms via `Profile::defaults` on next load). Negative values
    // collapse to 0, which the store accepts.
    let delay_str = editor.paste_char_delay_ms.read(cx).value().to_string();
    profile.paste_char_delay_ms = delay_str.trim().parse::<i32>().ok().map(|v| v.max(0));
    profile.highlight = editor.highlight;
    // The override flag → `Option` shape: false collapses to None
    // (inherit global); true persists the current vec, even if it's
    // empty (an explicit "no packs at all for this profile" state).
    profile.enabled_highlight_presets = if editor.override_highlight_packs {
        Some(editor.enabled_highlight_packs.clone())
    } else {
        None
    };
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
        && a.line_numbers == b.line_numbers
        && a.log_enabled == b.log_enabled
        && a.auto_reconnect == b.auto_reconnect
        && a.paste_warn_multiline == b.paste_warn_multiline
        && a.paste_slow == b.paste_slow
        && a.paste_char_delay_ms == b.paste_char_delay_ms
        && a.theme_id == b.theme_id
        && a.highlight == b.highlight
        && a.enabled_highlight_presets == b.enabled_highlight_presets
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
fn pill_button(s: SkinTokens, label: &'static str, danger: bool) -> gpui::Div {
    let fg = if danger { rgba(s.danger) } else { rgba(s.fg_primary) };
    let hover_bg = s.bg_input_hover;
    div()
        .px_3()
        .py_1()
        .bg(rgba(s.bg_input))
        .text_color(fg)
        .text_size(px(13.0))
        // Flex + items_center centers the glyph line-box within
        // the pill's padding box. Without this the text sits at
        // the top of the box because gpui's default line-height
        // is taller than the glyph itself and the leading
        // accumulates above rather than around the cap height.
        .flex()
        .items_center()
        .justify_center()
        .rounded_md()
        .cursor_pointer()
        .hover(move |st| st.bg(rgba(hover_bg)))
        .child(label)
}

/// Primary action button — solid `--accent` blue, white text. Used
/// for the form's Connect button (the call-to-action). Same shape
/// as `pill_button` so they line up flush in a button row.
fn primary_button(s: SkinTokens, label: &'static str) -> gpui::Div {
    div()
        .px_3()
        .py_1()
        .bg(rgba(s.accent))
        .text_color(rgba(s.accent_fg))
        .text_size(px(13.0))
        .flex()
        .items_center()
        .justify_center()
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
    line_numbers: bool,
    log_enabled: bool,
    auto_reconnect: bool,
    paste_warn_multiline: bool,
    paste_slow: bool,
    paste_char_delay_ms: Entity<InputState>,
    theme: Entity<SelectState<Vec<Opt>>>,
    highlight: bool,
    override_highlight_packs: bool,
    enabled_highlight_packs: Vec<String>,
    missing_drivers: Vec<crate::data::serial::chipsets::USBSerialCandidate>,
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
            line_numbers: e.line_numbers,
            log_enabled: e.log_enabled,
            auto_reconnect: e.auto_reconnect,
            paste_warn_multiline: e.paste_warn_multiline,
            paste_slow: e.paste_slow,
            paste_char_delay_ms: e.paste_char_delay_ms.clone(),
            theme: e.theme.clone(),
            highlight: e.highlight,
            override_highlight_packs: e.override_highlight_packs,
            enabled_highlight_packs: e.enabled_highlight_packs.clone(),
            missing_drivers: e.missing_drivers.clone(),
            error: e.error.clone(),
            scroll_handle: e.scroll_handle.clone(),
        }
    }
}

fn form_pane(
    er: EditorRender,
    packs: Vec<crate::data::highlight::HighlightPack>,
    global_enabled: Vec<String>,
    connected_session: bool,
    cx: &mut Context<AppView>,
) -> impl IntoElement {
    let s = *cx.global::<SkinTokens>();
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
        // No bg_main here — the cards inside paint `bg_panel`
        // directly over `bg_window` (the opaque shell), matching
        // Settings window's two-layer composition. With bg_main
        // here the panels would stack alpha and the form pane
        // ended up brighter / less grey than the rest of the
        // chrome.
        .text_color(rgba(s.fg_primary))
        .text_size(px(13.0))
        .flex()
        .flex_col()
        .child(form_header(
            er.is_edit,
            er.is_dirty,
            er.name.clone(),
            connected_session,
            cx,
        ))
        .child(form_body(er, packs, global_enabled, cx))
}

/// Header bar: editable profile name as the visible title (no
/// input chrome — `appearance(false)` strips the border/bg so it
/// reads as a heading rather than a form field), uppercase mode
/// tag underneath, action buttons on the right.
fn form_header(
    is_edit: bool,
    is_dirty: bool,
    name: Entity<InputState>,
    connected_session: bool,
    cx: &mut Context<AppView>,
) -> impl IntoElement {
    let s = *cx.global::<SkinTokens>();
    let subtitle = if is_edit { "EDIT PROFILE" } else { "NEW PROFILE" };
    // Save button text-color is the only thing that changes for the
    // dirty state — pill bg stays the same so the button doesn't
    // visually "appear" mid-edit. Tertiary fg (40% white) when
    // clean reads as "no-op available," primary fg (95% white)
    // when dirty reads as "click me to persist your changes."
    let save_fg = if is_dirty {
        rgba(s.fg_primary)
    } else {
        rgba(s.fg_tertiary)
    };
    let delete_btn = is_edit.then(|| {
        pill_button(s, "Delete", true).on_mouse_up(
            MouseButton::Left,
            cx.listener(|this, _: &MouseUpEvent, _window, cx| {
                this.delete_from_editor(cx);
            }),
        )
    });
    // Render the heading as a plain div instead of an Input —
    // gpui-component's Input fixes its own height to a small
    // `h_6` regardless of `Size::Size(_)`, which clipped 24px
    // text. Settings window's window_header uses the same plain-
    // div approach. Editing happens through a labeled "NAME"
    // field at the top of the Connection card now.
    let title_text = name.read(cx).value().to_string();
    let title_text = if title_text.is_empty() {
        "(unnamed)".to_string()
    } else {
        title_text
    };
    div()
        .w_full()
        .px_6()
        .py_3()
        .border_b_1()
        .border_color(rgba(s.border_subtle))
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
                    div()
                        .text_size(px(24.0))
                        .text_color(rgba(s.fg_primary))
                        .child(title_text),
                )
                .child(
                    div()
                        .text_size(px(10.0))
                        .text_color(rgba(s.fg_tertiary))
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
                    pill_button(s, "Save", false)
                        .text_color(save_fg)
                        .on_mouse_up(
                            MouseButton::Left,
                            cx.listener(|this, _: &MouseUpEvent, _window, cx| {
                                this.save_editor(cx);
                            }),
                        ),
                )
                .child(pill_button(s, "Cancel", false).on_mouse_up(
                    MouseButton::Left,
                    cx.listener(|this, _: &MouseUpEvent, _window, cx| {
                        this.cancel_editor(cx);
                    }),
                ))
                .when(connected_session, |row| {
                    // Suspended on the connected profile — swap the
                    // Connect button for the Disconnect + Resume
                    // pair Tauri shows in the same state. Connect on
                    // an already-connected profile would either
                    // race for its own port or re-open a session
                    // the user already has, neither of which is
                    // what they're after.
                    row.child(pill_button(s, "Disconnect", false).on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|this, _: &MouseUpEvent, window, cx| {
                            this.disconnect_current(window, cx);
                        }),
                    ))
                    .child(primary_button(s, "Resume").on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|this, _: &MouseUpEvent, window, cx| {
                            this.resume_session(window, cx);
                        }),
                    ))
                })
                .when(!connected_session, |row| {
                    row.child(primary_button(s, "Connect").on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|this, _: &MouseUpEvent, window, cx| {
                            this.save_and_connect(window, cx);
                        }),
                    ))
                }),
        )
}

/// Form body: a left rail of sub-tabs (Connection / Advanced) +
/// the active tab's content. Tab content is capped to a fixed
/// width so the cards keep form-shaped proportions on a wide
/// window. Mirrors the Tauri form's layout one-for-one.
fn form_body(
    er: EditorRender,
    packs: Vec<crate::data::highlight::HighlightPack>,
    global_enabled: Vec<String>,
    cx: &mut Context<AppView>,
) -> impl IntoElement {
    let s = *cx.global::<SkinTokens>();
    let active = er.tab;
    let content: gpui::AnyElement = match er.tab {
        EditorTab::Connection => div()
            .flex()
            .flex_col()
            .gap_3()
            .child(connection_card(
                er.name.clone(),
                er.port,
                er.baud,
                er.data_bits,
                er.parity,
                er.stop_bits,
                er.flow_control,
                er.missing_drivers.clone(),
                cx,
            ))
            .child(terminal_card(
                er.line_ending,
                er.backspace_key,
                er.local_echo,
                cx,
            ))
            .child(theme_card(s, er.theme))
            .into_any_element(),
        EditorTab::Highlighting => highlighting_pane(
            er.highlight,
            er.override_highlight_packs,
            er.enabled_highlight_packs.clone(),
            packs,
            global_enabled,
            cx,
        )
        .into_any_element(),
        EditorTab::Advanced => advanced_pane(
            er.dtr_on_connect,
            er.rts_on_connect,
            er.dtr_on_disconnect,
            er.rts_on_disconnect,
            er.hex_view,
            er.timestamps,
            er.line_numbers,
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
                                        .text_color(rgba(s.sidebar_error))
                                        .child(err)
                                })),
                        ),
                )
                .vertical_scrollbar(&er.scroll_handle),
        )
}

/// Left-rail sub-tab navigation. Each entry is a clickable row;
/// the active one paints `--bg-active` (translucent blue) so the
/// selected state reads instantly.
fn form_tab_nav(active: EditorTab, cx: &mut Context<AppView>) -> impl IntoElement {
    let s = *cx.global::<SkinTokens>();
    let item = move |label: &'static str, tab: EditorTab| {
        let is_active = tab == active;
        let bg = if is_active {
            rgba(s.bg_active)
        } else {
            rgba(0x00000000)
        };
        let fg = if is_active {
            rgba(s.fg_primary)
        } else {
            rgba(s.fg_secondary)
        };
        let hover_bg = s.bg_input;
        div()
            .w_full()
            .px_3()
            .py(px(6.0))
            .rounded_md()
            .bg(bg)
            .text_color(fg)
            .cursor_pointer()
            .hover(move |st| st.bg(rgba(hover_bg)))
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
        .border_color(rgba(s.border_subtle))
        .flex()
        .flex_col()
        .gap_1()
        .text_size(px(13.0))
        .child(item("Connection", EditorTab::Connection))
        .child(item("Highlighting", EditorTab::Highlighting))
        .child(item("Advanced", EditorTab::Advanced))
}

/// One section of the form — a translucent panel with a heading,
/// optional description, and a body. Section title size is
/// `--font-size-section` (15px); description is the muted
/// `--fg-secondary`. Panel uses `--radius-lg` (10px) and
/// `--bg-panel` / `--border-subtle`.
fn section_card(s: SkinTokens, title: &'static str, body: impl IntoElement) -> gpui::Div {
    section_card_with_desc(s, title, None, body)
}

fn section_card_with_desc(
    s: SkinTokens,
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
                .text_color(rgba(s.fg_primary))
                .child(title),
        );
    if let Some(desc) = description {
        header = header.child(
            div()
                .text_size(px(12.0))
                .text_color(rgba(s.fg_secondary))
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
        .bg(rgba(s.bg_panel))
        .border_1()
        .border_color(rgba(s.border_subtle))
        .rounded(px(s.radius_lg))
        // macOS 26 / Tahoe-style raised card. Matches the same
        // shadow_sm used by `settings_view::section_card_with_desc`
        // so the profile editor and the Settings window read at the
        // same elevation.
        .shadow_sm()
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
fn labeled(s: SkinTokens, label: &'static str, widget: impl IntoElement) -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .text_size(px(11.0))
                .text_color(rgba(s.fg_secondary))
                .whitespace_nowrap()
                .child(label),
        )
        .child(widget)
}

/// Detect unenrolled USB-serial adapters on platforms that need
/// vendor drivers. macOS / Windows have a real implementation in
/// `data::serial::detect`; Linux relies on kernel-side driver
/// loading (`pl2303.ko`, `ftdi_sio.ko`, `cp210x.ko`, …) and has no
/// equivalent missing-driver scenario, so it returns empty.
fn detect_missing_drivers() -> Vec<crate::data::serial::chipsets::USBSerialCandidate> {
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    {
        crate::data::serial::detect::detect_suspect_enumerated_ports()
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        Vec::new()
    }
}

/// One driver-not-loaded banner row. Matches the Tauri layout:
/// yellow `!` icon on the left, chipset + reason / product / serial
/// in the middle, and an "Install driver…" pill on the right when
/// the chipset entry carries a vendor URL. Clicking the pill opens
/// the URL in the user's default browser via `cx.open_url`.
fn driver_banner_row(
    s: SkinTokens,
    candidate: crate::data::serial::chipsets::USBSerialCandidate,
    cx: &mut Context<AppView>,
) -> gpui::Div {
    // Secondary line: prefer real product strings, but skip the
    // "please install…" / counterfeit placeholders the suspect-
    // product detector keys off — they're noise once we've already
    // resolved a chipset name above.
    let lower = candidate.product.to_lowercase();
    let product_is_placeholder = lower.contains("please install")
        || lower.contains("please download")
        || lower.contains("support windows")
        || lower.contains("counterfeit")
        || lower.contains("not support");
    let mut meta = if !candidate.product.is_empty() && !product_is_placeholder {
        candidate.product.clone()
    } else if !candidate.manufacturer.is_empty() {
        candidate.manufacturer.clone()
    } else {
        "USB device".to_string()
    };
    if !candidate.serial_number.is_empty() {
        meta.push_str(" \u{00B7} serial ");
        meta.push_str(&candidate.serial_number);
    }
    let title = format!("{} detected \u{2014} driver not loaded", candidate.chipset);

    let mut text_col = div().flex_1().min_w_0().flex().flex_col().gap_1().child(
        div()
            .text_size(px(13.0))
            .text_color(rgba(s.fg_primary))
            .child(title),
    );
    if !candidate.reason.is_empty() {
        text_col = text_col.child(
            div()
                .text_size(px(11.0))
                .text_color(rgba(s.fg_secondary))
                .whitespace_normal()
                .child(candidate.reason.clone()),
        );
    }
    text_col = text_col.child(
        div()
            .text_size(px(11.0))
            .text_color(rgba(s.fg_tertiary))
            .whitespace_normal()
            .child(meta),
    );

    let mut row = div()
        .w_full()
        .px_3()
        .py_2()
        .rounded_md()
        .border_1()
        .border_color(rgba(0xE3A93A55u32))
        .bg(rgba(0xE3A93A18u32))
        .flex()
        .flex_row()
        .items_center()
        .gap_3()
        .child(
            div()
                .w(px(22.0))
                .h(px(22.0))
                .rounded_full()
                .bg(rgba(0xE3A93AFFu32))
                .text_color(rgba(0x1A1A1AFFu32))
                .text_size(px(14.0))
                .flex()
                .items_center()
                .justify_center()
                .child("!"),
        )
        .child(text_col);
    if !candidate.driver_url.is_empty() {
        let url = candidate.driver_url.clone();
        row = row.child(pill_button(s, "Install driver\u{2026}", false).on_mouse_up(
            MouseButton::Left,
            cx.listener(move |_, _: &MouseUpEvent, _, cx| {
                cx.open_url(&url);
            }),
        ));
    }
    row
}

#[allow(clippy::too_many_arguments)]
fn connection_card(
    name: Entity<InputState>,
    port: Entity<SelectState<Vec<Opt>>>,
    baud: Entity<SelectState<Vec<Opt>>>,
    data_bits: Entity<SelectState<Vec<Opt>>>,
    parity: Entity<SelectState<Vec<Opt>>>,
    stop_bits: Entity<SelectState<Vec<Opt>>>,
    flow_control: Entity<SelectState<Vec<Opt>>>,
    missing_drivers: Vec<crate::data::serial::chipsets::USBSerialCandidate>,
    cx: &mut Context<AppView>,
) -> gpui::Div {
    let s = *cx.global::<SkinTokens>();
    let hover_bg = s.bg_input_hover;
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
                .child(labeled(s, "SERIAL PORT", Select::new(&port))),
        )
        .child(
            div()
                .px_2()
                .py_1()
                .bg(rgba(s.bg_input))
                .text_color(rgba(s.fg_primary))
                .rounded_md()
                .cursor_pointer()
                .hover(move |st| st.bg(rgba(hover_bg)))
                .child("↻")
                .on_mouse_up(
                    MouseButton::Left,
                    cx.listener(|this, _: &MouseUpEvent, window, cx| {
                        this.rescan_ports(window, cx);
                    }),
                ),
        );
    // Build a "driver not loaded" banner per detected candidate so
    // an unenrolled CP210x / FTDI / PL2303 / CH340 shows up RIGHT
    // ABOVE the Serial Port picker — same shape the Tauri version
    // uses. Hidden when `Settings::disable_driver_detection` is on
    // (build_editor passes an empty Vec).
    let driver_banners: Vec<gpui::Div> = missing_drivers
        .into_iter()
        .map(|d| driver_banner_row(s, d, cx))
        .collect();
    let body = div()
        .flex()
        .flex_col()
        .gap_3()
        .child(labeled(s, "NAME", Input::new(&name).appearance(true)))
        .when(!driver_banners.is_empty(), |this| {
            this.child(
                div().flex().flex_col().gap_2().children(driver_banners),
            )
        })
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
                        .child(labeled(s, "BAUD RATE", Select::new(&baud))),
                )
                .child(
                    div()
                        .flex_1()
                        .child(labeled(s, "DATA BITS", Select::new(&data_bits))),
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
                        .child(labeled(s, "PARITY", Select::new(&parity))),
                )
                .child(
                    div()
                        .flex_1()
                        .child(labeled(s, "STOP BITS", Select::new(&stop_bits))),
                ),
        )
        .child(labeled(s, "FLOW CONTROL", Select::new(&flow_control)));
    section_card(s, "Connection", body)
}

fn terminal_card(
    line_ending: Entity<SelectState<Vec<Opt>>>,
    backspace_key: Entity<SelectState<Vec<Opt>>>,
    local_echo: bool,
    cx: &mut Context<AppView>,
) -> gpui::Div {
    let s = *cx.global::<SkinTokens>();
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
                            s,
                            "SEND LINE ENDING",
                            Select::new(&line_ending),
                        )),
                )
                .child(
                    div()
                        .flex_1()
                        .child(labeled(
                            s,
                            "BACKSPACE SENDS",
                            Select::new(&backspace_key),
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
    section_card(s, "Terminal", body)
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
    let s = *cx.global::<SkinTokens>();
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
                .text_color(rgba(s.fg_secondary))
                .child(h),
        );
    }
    row
}

/// Profile-level Highlighting tab. Two cards: the master toggle that
/// enables/disables highlighting for this profile (mirrors
/// `Profile::highlight`), and the per-pack list with an "Override
/// global" switch on top. When the override is off, the per-pack
/// rows are still shown but read-only, so the user can see what
/// they're inheriting from Settings without flipping the switch.
fn highlighting_pane(
    highlight: bool,
    override_packs: bool,
    enabled_packs: Vec<String>,
    packs: Vec<crate::data::highlight::HighlightPack>,
    global_enabled: Vec<String>,
    cx: &mut Context<AppView>,
) -> impl IntoElement {
    let s = *cx.global::<SkinTokens>();

    // When override is off, the rows mirror the global pick; when
    // on, they reflect the per-profile working list. Disabled state
    // is the same in either case — toggling needs override + master.
    let effective: &Vec<String> = if override_packs {
        &enabled_packs
    } else {
        &global_enabled
    };

    let rows: Vec<gpui::Div> = packs
        .into_iter()
        .map(|p| {
            let id_for_setter = p.id.clone();
            let cb_id = SharedString::from(format!("profile-highlight-{}", p.id));
            let is_on = effective.iter().any(|e| e == &p.id);
            let label = if p.source == "user" || p.source == "import" {
                format!("{} (custom)", p.name)
            } else {
                p.name.clone()
            };
            let desc = p
                .description
                .clone()
                .filter(|d| !d.is_empty())
                .unwrap_or_else(|| "\u{2014}".to_string());
            let cb = Checkbox::new(cb_id)
                .checked(is_on)
                .disabled(!override_packs || !highlight)
                .label(label)
                .on_click(cx.listener(move |this, checked: &bool, _, cx| {
                    this.toggle_editor_highlight_pack(
                        id_for_setter.clone(),
                        *checked,
                        cx,
                    );
                }));
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(cb)
                .child(
                    div()
                        .pl(px(24.0))
                        .text_size(px(12.0))
                        .text_color(rgba(s.fg_secondary))
                        .whitespace_normal()
                        .child(desc),
                )
        })
        .collect();

    let master_card = section_card_with_desc(
        s,
        "Highlighting",
        Some(
            "Master switch for this profile. When off, incoming \
             output is rendered without any rule-based colouring \
             regardless of which packs are enabled below.",
        ),
        bool_field(
            "profile-highlight-master",
            "Highlight terminal output for this profile",
            highlight,
            cx,
            |ed, on| ed.highlight = on,
        ),
    );

    // Override toggle goes directly to AppView so it can seed the
    // per-profile list from the global on first-on (otherwise the
    // user flips the switch and silently loses their inherited
    // selection).
    let override_row = div().flex().flex_row().items_center().gap_3().child(
        Checkbox::new("profile-highlight-override")
            .checked(override_packs)
            .disabled(!highlight)
            .label("Override global pack selection")
            .on_click(cx.listener(|this, checked: &bool, _, cx| {
                this.set_editor_override_highlight(*checked, cx);
            })),
    );

    let packs_card = section_card_with_desc(
        s,
        "Highlight Packs",
        Some(
            "Inherit the global selection from Settings, or override \
             it for this profile. With override off, the rows show \
             what the global is currently broadcasting (read-only).",
        ),
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(override_row)
            .child(div().flex().flex_col().gap_2().children(rows)),
    );

    div()
        .flex()
        .flex_col()
        .gap_3()
        .child(master_card)
        .child(packs_card)
}

#[allow(clippy::too_many_arguments)]
#[allow(clippy::too_many_arguments)]
fn advanced_pane(
    dtr_on_connect: Entity<SelectState<Vec<Opt>>>,
    rts_on_connect: Entity<SelectState<Vec<Opt>>>,
    dtr_on_disconnect: Entity<SelectState<Vec<Opt>>>,
    rts_on_disconnect: Entity<SelectState<Vec<Opt>>>,
    hex_view: bool,
    timestamps: bool,
    line_numbers: bool,
    log_enabled: bool,
    auto_reconnect: bool,
    paste_warn_multiline: bool,
    paste_slow: bool,
    paste_char_delay_ms: Entity<InputState>,
    cx: &mut Context<AppView>,
) -> impl IntoElement {
    let s = *cx.global::<SkinTokens>();
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
                        .text_color(rgba(s.fg_primary))
                        .child("Advanced"),
                )
                .child(
                    div()
                        .text_size(px(12.0))
                        .text_color(rgba(s.fg_secondary))
                        .whitespace_normal()
                        .child("Control lines, hex view, timestamps, session logging."),
                ),
        )
        .child(control_lines_card(
            s,
            dtr_on_connect,
            rts_on_connect,
            dtr_on_disconnect,
            rts_on_disconnect,
        ))
        .child(output_card(
            hex_view,
            timestamps,
            line_numbers,
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

/// Per-profile theme override card. The `theme` Select's first
/// option is "Use global default" with an empty id; saving with
/// that selected leaves `Profile::theme_id` empty, which
/// `compute_palette` treats as fall-through to
/// `Settings::default_theme_id`. The same Select otherwise lists
/// every theme `themes_store` knows about.
fn theme_card(s: SkinTokens, theme: Entity<SelectState<Vec<Opt>>>) -> gpui::Div {
    section_card_with_desc(
        s,
        "Terminal Theme",
        Some(
            "Override the global default theme just for this profile. \
             Useful for keeping different palettes on different \
             devices (e.g. red-tinged for production routers, calm \
             green for the lab switch). Leave on \"Use global \
             default\" to inherit from Settings \u{2192} Themes.",
        ),
        Select::new(&theme),
    )
}

fn control_lines_card(
    s: SkinTokens,
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
            .child(div().flex_1().child(labeled(s, left_label, left)))
            .child(div().flex_1().child(labeled(s, right_label, right)))
    };
    let body = div()
        .flex()
        .flex_col()
        .gap_3()
        .child(row(
            "DTR ON CONNECT",
            Select::new(&dtr_on_connect),
            "RTS ON CONNECT",
            Select::new(&rts_on_connect),
        ))
        .child(row(
            "DTR ON DISCONNECT",
            Select::new(&dtr_on_disconnect),
            "RTS ON DISCONNECT",
            Select::new(&rts_on_disconnect),
        ));
    section_card_with_desc(
        s,
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
    line_numbers: bool,
    log_enabled: bool,
    auto_reconnect: bool,
    cx: &mut Context<AppView>,
) -> gpui::Div {
    let s = *cx.global::<SkinTokens>();
    // Single column. The 2-col grid for Hex view + Line timestamps
    // produced an awkward orphan-feel where the right-hand "Line
    // timestamps" hint wrapped while the others were full-width;
    // stacking reads cleaner and matches the rest of the card's
    // vertical rhythm.
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
            "line-numbers",
            "Line numbers",
            Some("prefix each line with a session-local counter (resets on reconnect)"),
            line_numbers,
            cx,
            |ed, v| ed.line_numbers = v,
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
    section_card(s, "Output", body)
}

fn paste_safety_card(
    paste_warn_multiline: bool,
    paste_slow: bool,
    paste_char_delay_ms: Entity<InputState>,
    cx: &mut Context<AppView>,
) -> gpui::Div {
    let s = *cx.global::<SkinTokens>();
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
                    s,
                    "SLOW-PASTE DELAY (MS)",
                    Input::new(&paste_char_delay_ms).small().appearance(true),
                )),
        );
    section_card_with_desc(
        s,
        "Paste safety",
        Some(
            "Catch the \"I pasted into the wrong window\" mistake, and pace pastes so \
             UARTs on slower devices don't drop bytes.",
        ),
        body,
    )
}

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
    fn color(self, tokens: SkinTokens) -> u32 {
        match self {
            RowStatus::Connected => tokens.success,
            RowStatus::Reconnecting => tokens.warn,
            RowStatus::Failed => tokens.sidebar_error,
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
    let tokens = *cx.global::<SkinTokens>();
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
        rgba(tokens.bg_hover)
    } else {
        rgba(tokens.bg_sidebar)
    };
    let hover_bg = tokens.bg_hover;

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
        .child(div().text_color(rgba(tokens.fg_primary)).child(name))
        .children(status.map(|st| {
            let dot = div()
                .w(px(STATUS_DOT_PX))
                .h(px(STATUS_DOT_PX))
                .rounded_full()
                .bg(rgba(st.color(tokens)));
            if st == RowStatus::Reconnecting
                && !cx.global::<crate::ReduceMotion>().0
            {
                // Same 1s pulse as the session header so the two
                // indicators feel like one signal. Animation name
                // is per-row-instance to avoid gpui de-duping
                // animations across distinct dots. Skipped under
                // prefers-reduced-motion — the orange dot's
                // colour already communicates the reconnect state.
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
        .hover(move |st| st.bg(rgba(hover_bg)))
        .cursor_pointer()
        .flex()
        .flex_col()
        .gap_1()
        .child(header)
        .child(
            div()
                .text_size(px(11.0))
                .text_color(rgba(tokens.fg_tertiary))
                .child(port),
        );
    if let Some(err) = error {
        row = row.child(
            div()
                .text_size(px(11.0))
                .text_color(rgba(tokens.sidebar_error))
                .child(err),
        );
    }
    let id_for_left = id.clone();
    let id_for_right = id;
    row.on_mouse_up(
        MouseButton::Left,
        cx.listener(move |this, _: &MouseUpEvent, window, cx| {
            this.select_profile(id_for_left.clone(), window, cx);
        }),
    )
    .on_mouse_down(
        MouseButton::Right,
        cx.listener(move |this, evt: &MouseDownEvent, _, cx| {
            this.open_profile_context_menu(id_for_right.clone(), evt.position, cx);
        }),
    )
}
