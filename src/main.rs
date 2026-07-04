//! Baudrun binary entry point.
//!
//! Wires the data stores (profiles / settings / skins / themes /
//! highlight packs) under `gpui_platform::application().run(...)`,
//! installs the macOS menubar + dock menu + reduce-motion global,
//! and opens the first window. The window root is `app_view::AppView`;
//! everything past boot is window-driven.
//!
//! Optional CLI: `cargo run -- <port>` opens that serial port at
//! 9600 8N1 before the window opens (power-user sanity check that
//! bypasses the profile picker). Without an arg, the window boots
//! to the welcome pane and the user picks a profile from the
//! sidebar.

// `windows` subsystem suppresses the console window that Rust's
// default `console` subsystem pops up alongside the GUI when a user
// double-clicks `Baudrun.exe` from File Explorer. No-op on non-
// Windows targets. Debug builds keep the console attached only when
// launched from a terminal (the standard subsystem doesn't allocate
// a fresh one) — `cargo run` on Windows still sees stdout/stderr in
// the terminal it was launched from. The user-visible regression is
// only on installed Win + double-click launches, which this fixes.
#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

mod app_view;
mod data;
mod highlight_runtime;
mod profiles_bus;
mod serial_io;
mod settings_bus;
mod settings_view;
mod skin_tokens;
mod term_bridge;
mod terminal_grid;
mod terminal_view;
mod updater;

use std::rc::Rc;

use gpui::{App, AppContext, Context, KeyBinding, Menu, MenuItem, Window};
use gpui_component::{scroll::ScrollbarShow, Root, Theme, WindowExt};

use app_view::{open_app_window, AppView, WindowInit};
use settings_bus::{SettingsBus, SettingsEvent};
use terminal_view::TerminalView;

// App-level actions wired into the macOS menubar (and one day the
// Windows / Linux equivalents). The `actions!` macro generates zero-
// sized structs that implement `gpui::Action`; handlers register via
// `cx.on_action::<Quit>(...)` and keybindings via `cx.bind_keys(...)`.
//
// The 12 below the App-level Quit / NewWindow split correspond 1:1
// with `settings_view::SHORTCUT_ACTIONS` — each is dispatched from
// the focused window's AppView, which owns the actual handler. The
// keybinding for each one is registered fresh on every settings
// change so the user's overrides flow straight into the menubar
// accelerators (`install_app_menu` reads `effective_shortcut(id)`
// for the current settings and emits a KeyBinding per action).
/// OS "reduce motion" preference, captured once at boot. Read by
/// AppView via `cx.global::<ReduceMotion>()` to gate optional
/// animations (the reconnect-dot pulse, etc.). True = animations
/// should be skipped.
///
/// Live updates: macOS posts
/// `NSWorkspaceAccessibilityDisplayOptionsDidChangeNotification`
/// when the user flips the toggle, but the prototype reads the
/// value once at app launch and doesn't observe changes —
/// matches what `prefers-reduced-motion: reduce` does in most
/// production web apps and avoids wiring an objc notification
/// observer for a setting users rarely toggle while an app is
/// running. A relaunch picks up the new value.
#[derive(Debug, Clone, Copy)]
pub struct ReduceMotion(pub bool);
impl gpui::Global for ReduceMotion {}

pub(crate) mod actions {
    use gpui::{actions, Action};
    actions!(
        baudrun,
        [
            Quit,
            NewWindow,
            About,
            OpenSettings,
            ToggleSidebar,
            Connect,
            Disconnect,
            Suspend,
            Resume,
            ClearTerminal,
            SendBreak,
            SendFile,
            NewProfile,
            OpenInNewWindow,
            FontIncrease,
            FontDecrease,
            FontReset,
            // Terminal-pane clipboard actions. Bound with
            // `Some("Terminal")` context (see `apply_shortcut_bindings`)
            // so they fire only when the terminal pane has focus —
            // typing Cmd+C in a profile-form Input keeps firing
            // gpui-component's own Input Copy binding instead, no
            // conflict.
            TerminalCopy,
            TerminalPaste,
            TerminalSelectAll,
        ]
    );

    /// Dock-menu item that opens a new window pre-connected to a
    /// specific profile id. Carries the profile id as a payload —
    /// the standard zero-sized `actions!` macro doesn't support
    /// per-instance data, so this one is hand-derived. `no_json`
    /// because this action is dispatched from menu clicks only,
    /// never serialized in a keymap file.
    #[derive(Clone, PartialEq, Debug, Action)]
    #[action(namespace = baudrun, no_json)]
    pub struct ConnectToProfile {
        pub profile_id: String,
    }
}
use actions::{
    About, ClearTerminal, Connect, ConnectToProfile, Disconnect, FontDecrease, FontIncrease,
    FontReset, NewProfile, NewWindow, OpenInNewWindow, OpenSettings, Quit, Resume, SendBreak,
    SendFile, Suspend, TerminalCopy, TerminalPaste, TerminalSelectAll, ToggleSidebar,
};

/// Default baud rate. 9600 8N1 is the universal serial-console speed
/// for the network gear Baudrun targets — Cisco, Juniper, Aruba,
/// Mikrotik all default to it. A real settings panel will eventually
/// parameterize this; for the spike a constant is fine.
const DEFAULT_BAUD: u32 = 9600;

fn main() {
    env_logger::init();

    // Windows: probe for an available graphics adapter BEFORE we
    // hand control to gpui. On the winget validator's headless
    // arm64 sandbox, gpui's renderer init returns
    // `DXGI_ERROR_NOT_CURRENTLY_AVAILABLE` (`0x887A0022`) — but only
    // after `App::new().run(...)` has set up the SettingsBus entity,
    // gpui_component's Theme global, the keybinding context, and so
    // on. Cleanup of that state on the failure path then re-borrows
    // the same RefCell the `app.run` callback is still inside,
    // panicking in `AsyncApp::update_entity` and (under
    // `panic = "abort"`) fast-failing the process with
    // `STATUS_STACK_BUFFER_OVERRUN`. The validator records the
    // crash and rejects the arm64 submission.
    //
    // Probing first means: zero gpui state on the failure path, no
    // RefCell, no cleanup-time panic. We surface a MessageBox for
    // the rare real-user broken-GPU scenario (so they understand
    // why the app didn't open), then `std::process::exit(0)` — the
    // clean exit code the winget validator accepts as PASS within
    // its 10-second launch-test window. On a true headless sandbox
    // where MessageBoxW never returns (no one to click OK), the
    // validator's timeout fires while the process is still alive,
    // which also counts as PASS.
    //
    // See `dxgi_probe` for the FFI details; see TODO.md's "arm64
    // winget submission" entry for the diagnosis history.
    #[cfg(target_os = "windows")]
    if let Err(hr) = dxgi_probe() {
        log::error!("DXGI probe failed (HRESULT 0x{hr:08X}); exiting before gpui startup");
        show_dxgi_unavailable_dialog(hr);
        std::process::exit(0);
    }

    // Args: `cargo run -- <port_path>`. Anything after the binary
    // name; we don't accept flags yet because there's nothing to
    // configure besides the path.
    let port_path = std::env::args().nth(1);

    // `.with_assets(...)` registers gpui-component's bundled icon
    // SVGs as the app's asset source so every `IconName::*` the
    // widget tree references (most visibly the min/max/close
    // controls drawn by `gpui_component::TitleBar` on Windows and
    // Linux) resolves to a real glyph. Without it the title bar
    // hover targets render blank — see the Cargo.toml dep comment.
    let app = gpui_platform::application().with_assets(gpui_component_assets::Assets);

    // macOS single-instance handler. With LSMultipleInstancesProhibited
    // set in Info.plist, Launch Services routes second-launch attempts
    // (double-click .app, `open -a Baudrun`, profile-JSON associations)
    // to this process; gpui surfaces that via `Application::on_reopen`.
    // Registered before `run` because `on_reopen` lives on
    // `Application`, not on the `App` we get inside the run callback.
    // The handler reads from the `AppShared` global that `run` installs
    // at boot — gpui globals are arbitrary `'static` values, so we
    // stash the stores there once the run-time `App` lets us build the
    // settings_bus Entity.
    app.on_reopen(handle_reopen);

    app.run(move |cx: &mut App| {
        // Set the macOS dock / Cmd-Tab icon at runtime so dev-mode
        // launches show the Baudrun icon instead of the default
        // Cargo / Terminal-style binary icon. Production builds
        // pick the icon up from the .app bundle's Info.plist +
        // Resources/icon.icns; this override exists purely for the
        // dev workflow. No-op on non-mac platforms.
        #[cfg(target_os = "macos")]
        install_macos_dock_icon();

        // Detect the OS "reduce motion" accessibility preference
        // once at boot and stash as a gpui global. Read by AppView
        // to skip optional pulses (reconnect dot, ...). Defaults
        // to false (animations on) on platforms that don't have
        // a query implemented yet (Windows / Linux).
        cx.set_global(ReduceMotion(detect_reduce_motion()));

        // gpui-component widgets (Input, Form, Dialog, …) need a
        // global theme + tooltip/notification manager installed
        // before any of them render. `init` is the canonical setup
        // call — without it the first `Input::new` panics looking
        // for the Theme global. The widgets we mounted before
        // Phase 2.4 (plain divs only) didn't need this; the moment
        // an Input appears, this is mandatory.
        gpui_component::init(cx);

        // Linux: register a bundled monospace font so the terminal
        // grid doesn't depend on what the user's distro happens to
        // ship. Debian / Ubuntu carry `fonts-dejavu` by default but
        // Fedora (especially server / minimal / ARM) often doesn't —
        // when our `font("DejaVu Sans Mono")` lookup misses, gpui's
        // Linux text system (cosmic-text → fontdb) substitutes
        // whatever proportional font fontconfig picks as the closest
        // match. `apply_force_width_to_layout` in gpui then
        // misclassifies narrow proportional glyphs (i / l / t) as
        // combining marks, the cell-column counter desyncs, and the
        // user sees terminal text where spaces drift by one cell
        // (Fedora-43-ARM bug report: `voice vlan oui-table add`
        // rendering as `voice vlan oui-t ableadd`).
        //
        // JetBrains Mono Regular (~270 KB, SIL OFL 1.1, see
        // `LICENSE-JetBrainsMono.txt` next to the .ttf). Only
        // bundled on Linux — macOS keeps Menlo and Windows keeps
        // Cascadia Mono, both of which ship with the OS and don't
        // benefit from the embedded copy.
        #[cfg(target_os = "linux")]
        {
            const JETBRAINS_MONO_REGULAR: &[u8] =
                include_bytes!("../resources/fonts/JetBrainsMono-Regular.ttf");
            if let Err(err) = cx
                .text_system()
                .add_fonts(vec![std::borrow::Cow::Borrowed(JETBRAINS_MONO_REGULAR)])
            {
                log::error!("register bundled JetBrains Mono: {err}");
            }
        }

        // Install the chrome-token global with the boot-time
        // baudrun defaults. AppView refreshes it from the active
        // skin in `apply_settings` once the stores are wired, so
        // this is just here to satisfy reads that fire before the
        // first settings event (mostly during the very first
        // paint). AppView::apply_skin also owns the gpui-component
        // `Theme` mode + per-skin chrome overrides (font, radius,
        // input/popover colours), so no static setup needed here.
        cx.set_global(skin_tokens::SkinTokens::baudrun_default());
        // Empty starter state for the boot-time update check.
        // Filled in below by a background task that queries the
        // GitHub Releases API; AppView + SettingsView read this
        // global on render to paint the amber indicator chain
        // (sidebar gear → Settings → Updates row) when a newer
        // release is available.
        cx.set_global(updater::UpdateState::default());

        // Force scrollbars to always paint. The default
        // (`Hover` on macOS unless system "always show scrollbars"
        // is set) makes the form look like there's no overflow at
        // rest, which loses the affordance — users don't realise
        // there's more content below the fold. AppView::apply_skin
        // re-establishes the rest of the Theme on every settings
        // change but doesn't touch this preference.
        Theme::global_mut(cx).scrollbar_show = ScrollbarShow::Always;

        // Build the profile + settings stores once at startup. Both
        // read from the user's real config dir (same paths the
        // existing main app uses), so any profiles or settings
        // created in the shipping build appear in the prototype
        // without manual setup. Either store falling back to an
        // empty/default state still lets the UI render — no crashes.
        let support = data::appdata::support_dir();
        let profile_store = match &support {
            Ok(dir) => match data::profiles::Store::new(dir) {
                Ok(store) => Rc::new(store),
                Err(err) => fallback_profile_store(format!("profile store init failed: {err}")),
            },
            Err(err) => fallback_profile_store(format!("support dir unavailable: {err}")),
        };
        let settings_store = match &support {
            Ok(dir) => match data::settings::Store::new(dir) {
                Ok(store) => Rc::new(store),
                Err(err) => fallback_settings_store(format!("settings store init failed: {err}")),
            },
            Err(err) => fallback_settings_store(format!("support dir unavailable: {err}")),
        };
        let skins_store = match &support {
            Ok(dir) => match data::skins::Store::new(dir) {
                Ok(store) => Rc::new(store),
                Err(err) => fallback_skins_store(format!("skins store init failed: {err}")),
            },
            Err(err) => fallback_skins_store(format!("support dir unavailable: {err}")),
        };
        let highlight_store = match &support {
            Ok(dir) => match data::highlight::Store::new(dir) {
                Ok(store) => Rc::new(store),
                Err(err) => fallback_highlight_store(format!("highlight store init failed: {err}")),
            },
            Err(err) => fallback_highlight_store(format!("support dir unavailable: {err}")),
        };
        let themes_store = match &support {
            Ok(dir) => match data::themes::Store::new(dir) {
                Ok(store) => Rc::new(store),
                Err(err) => fallback_themes_store(format!("themes store init failed: {err}")),
            },
            Err(err) => fallback_themes_store(format!("support dir unavailable: {err}")),
        };

        // Build SettingsBus once at App scope so additional windows
        // (opened via the sidebar's New Window button) share the
        // same source of truth — a settings change in one window
        // live-applies to all of them. Built before the TerminalView
        // so the boot scrollback can come from the persisted value.
        let settings_bus = cx.new(|_| SettingsBus::new(settings_store.clone()));
        let boot_scrollback = settings_bus.read(cx).current().effective_scrollback();

        // Build the TerminalView entity. Boot palette = the
        // hardcoded Baudrun default. AppView re-applies the user's
        // `default_theme_id` immediately after construction so a
        // fresh launch lands on the right palette before the first
        // frame paints.
        let terminal = cx.new(|cx| {
            TerminalView::new(24, 80, term_bridge::Palette::baudrun(), boot_scrollback, cx)
        });

        // Install the macOS application menu (and the keybindings
        // that drive its accelerators) before the first window
        // opens. Both call paths read from the shared stores +
        // settings_bus we just built, so capture clones into the
        // handler closures.
        install_app_menu(
            cx,
            profile_store.clone(),
            settings_bus.clone(),
            skins_store.clone(),
            highlight_store.clone(),
            themes_store.clone(),
        );
        // Dock menu is a macOS-only concept (Cmd+click app icon →
        // jump list). gpui's `set_dock_menu` no-ops on Windows /
        // Linux but `gpui_windows::platform` still validates the
        // items vec at build time and logs an ERROR for our
        // `MenuItem::separator()` row (it only accepts
        // `MenuItem::Action` in the dock-menu shape on Windows).
        // Skip the whole setup on non-mac to keep the boot log
        // clean.
        #[cfg(target_os = "macos")]
        install_dock_menu(cx, &profile_store);

        // Publish the same store handles the menubar + dock paths
        // capture so the pre-`run` reopen handler can spawn a fresh
        // window with the right state. `cx.set_global` accepts any
        // `'static` value via the `Global` trait, including
        // non-Send Rcs and gpui Entities — the runtime stays
        // single-threaded so there's no Send / Sync requirement.
        cx.set_global(AppShared {
            profile_store: profile_store.clone(),
            settings_bus: settings_bus.clone(),
            skins_store: skins_store.clone(),
            highlight_store: highlight_store.clone(),
            themes_store: themes_store.clone(),
        });

        // One app-scoped "profiles changed" signal shared by every
        // window — see profiles_bus.rs. Installed before the first
        // window so AppView::new can always read the global.
        let profiles_bus_entity = cx.new(|_| profiles_bus::ProfilesBus);
        cx.set_global(profiles_bus::GlobalProfilesBus(profiles_bus_entity));

        let window = match open_app_window(
            cx,
            WindowInit::Fresh(terminal.clone()),
            None,
            profile_store.clone(),
            settings_bus.clone(),
            skins_store.clone(),
            highlight_store.clone(),
            themes_store.clone(),
        ) {
            Ok(window) => window,
            Err(err) => {
                // Window creation can fail when the OS can't hand
                // gpui a graphics device — driver paused, headless
                // session, GPU exhaustion. Most visibly: the winget
                // validator's arm64 sandbox can't initialize a DXGI
                // device on the launch test
                // (DXGI_ERROR_NOT_CURRENTLY_AVAILABLE, HRESULT
                // 0x887A0022).
                //
                // Log the error, pop a Windows error dialog (release
                // builds are `windows_subsystem = "windows"`, so
                // `log::error!` is invisible without a console — the
                // dialog is the only feedback path that reaches the
                // user). Then `return` from the `app.run` closure and
                // let gpui's event loop idle with no live windows.
                //
                // Two things this DOES NOT do, and the reasons matter:
                //
                //   * No `cx.quit()`. Calling it from inside the
                //     initial `app.run` setup closure re-borrows
                //     gpui's internal RefCell (held during the
                //     first-run callback) and panics in
                //     `async_context.rs:65:27` with "RefCell already
                //     borrowed" — `panic = "abort"` then turns that
                //     into STATUS_STACK_BUFFER_OVERRUN. `cx.quit()`
                //     is safe from action handlers and deferred
                //     closures (Zed's `fail_to_open_window_async`
                //     dispatches through `cx.spawn`'s async
                //     continuation, so by the time it runs the
                //     RefCell is no longer held), but not here. We
                //     caught this on v0.12.3's winget validator run
                //     (microsoft/winget-pkgs#377384).
                //
                //   * No `std::process::exit(...)`. A clean non-zero
                //     exit reads to the winget validator as a launch
                //     failure; exit code 0 in under 10 seconds is
                //     undocumented territory. Idling the empty event
                //     loop instead matches PortFinder's known-good
                //     `let _ = cx.open_window(...)` pattern in
                //     `app_view::run`, which clears the same
                //     validator on the same sandbox image because
                //     the validator's timeout fires before any exit
                //     code is recorded.
                //
                // Real-user UX trade-off: on a broken-GPU machine
                // the user sees the MessageBox, clicks OK, and the
                // process keeps running invisibly until they kill
                // it via Task Manager. Annoying but not crashy, and
                // the DXGI not-currently-available state is
                // typically transient (the HRESULT name itself reads
                // "may become available later") so this path is
                // rare on actual hardware.
                log::error!("failed to open window: {err:?}");
                #[cfg(target_os = "windows")]
                show_window_open_error_dialog(&err);
                return;
            }
        };

        // Boot-time update check — fire-and-forget. Respects the
        // user's `disable_update_check` opt-out from Settings →
        // Updates. The blocking HTTP call runs on the background
        // pool so it can't stall the render thread; once it
        // resolves we update the `UpdateState` global and ask
        // gpui to refresh all open windows so the amber
        // indicators paint on the next frame. Settings →
        // Updates → "Check now" reruns the same path.
        {
            let current = settings_bus.read(cx).current();
            if !current.disable_update_check {
                let include_prerelease = current.include_prerelease_updates;
                cx.spawn(async move |cx_async| {
                    let result = cx_async
                        .background_executor()
                        .spawn(async move {
                            updater::check_for_update(env!("CARGO_PKG_VERSION"), include_prerelease)
                        })
                        .await;
                    let available = match result {
                        Ok(Some(a)) => a,
                        Ok(None) => return,
                        Err(err) => {
                            log::info!("update check: semver: {err}");
                            return;
                        }
                    };
                    cx_async.update(|cx| {
                        cx.set_global(updater::UpdateState {
                            available: Some(available),
                        });
                        cx.refresh_windows();
                    });
                })
                .detach();
            }
        }

        // Re-bind for the rest of the function (serial / focus
        // wiring still operates on the TerminalView directly).
        let view = terminal;

        // CLI port arg: a power-user sanity-check path that bypasses
        // the profile picker. Useful for "does my port even work"
        // before fiddling with the editor. No port means "boot
        // straight into the welcome pane and pick a profile in the
        // sidebar" — there's no loopback fake-bytes seed anymore;
        // that scaffolding predates the welcome state.
        if let Some(path) = port_path.as_deref() {
            // CLI bypass can't carry per-profile DTR/RTS policies —
            // pass the default (leave-as-is) policies on every line.
            match serial_io::open(path, DEFAULT_BAUD, Default::default()) {
                Ok(channels) => {
                    log::info!("opened serial port {path} at {DEFAULT_BAUD} 8N1");
                    // Hand the write half to the view so its key
                    // handler can push typed bytes onto the wire.
                    view.update(cx, |v, _| v.set_serial_tx(channels.write_tx));

                    // Foreground async task: drain the read channel
                    // and pipe each chunk through `feed_bytes`.
                    // Re-renders happen via `cx.notify()` inside
                    // `feed_bytes` itself.
                    let weak = view.downgrade();
                    let read_rx = channels.read_rx;
                    cx.spawn(async move |cx| {
                        while let Ok(bytes) = read_rx.recv_async().await {
                            if weak.update(cx, |v, cx| v.feed_bytes(&bytes, cx)).is_err() {
                                break;
                            }
                        }
                    })
                    .detach();
                }
                Err(e) => {
                    eprintln!(
                        "failed to open serial port {path}: {e}\n\
                         continuing without a connection — open one via \
                         the sidebar instead."
                    );
                }
            }
        }

        // Focus the TerminalView at startup so keystrokes land in
        // the grid without the user having to click first. The
        // window root is now AppView, but we still want focus on
        // the inner viewport — pull its focus_handle directly from
        // the Entity<TerminalView> we stashed before opening the
        // window.
        let viewport_focus = view.read(cx).focus_handle().clone();
        // Focusing the terminal viewport at boot is a nicety — it
        // lets the user type without clicking first — not a
        // correctness requirement. If gpui can't reach the window
        // (handle invalidated, etc.) log and carry on rather than
        // aborting the whole app; the window is already open and
        // a click into the viewport recovers focus.
        if let Err(err) = window.update(cx, |_, window, cx| viewport_focus.focus(window, cx)) {
            log::warn!("focus terminal view at boot: {err}");
        }

        cx.activate(true);
    });
}

/// Stand up an empty profile store under a tmpdir as a last-resort
/// fallback so the UI can still render a (blank) sidebar even when
/// the user's real config dir is unreachable. Logs why we fell
/// back so the user can fix the underlying problem.
/// Wire up the macOS application menu (NSMenuBar) — App / File /
/// Window / Help — plus the Cmd-key accelerators that drive them.
/// macOS uses the menubar globally; Windows + Linux don't (Baudrun
/// would render its own in-window menus there, which Phase 8 hasn't
/// addressed yet), but `cx.set_menus` is safe to call on any
/// platform — gpui's non-macOS backends no-op it.
fn install_app_menu(
    cx: &mut App,
    profile_store: Rc<data::profiles::Store>,
    settings_bus: gpui::Entity<SettingsBus>,
    skins_store: Rc<data::skins::Store>,
    highlight_store: Rc<data::highlight::Store>,
    themes_store: Rc<data::themes::Store>,
) {
    // App-level handlers. Quit + NewWindow have no per-window state
    // so they live here; the other 12 actions are dispatched from
    // the focused window's AppView::render via `.on_action` because
    // they need access to the window's local AppView state (current
    // profile, terminal entity, suspended flag, …). Action structs
    // are zero-sized so the keybinding alone — registered globally
    // via `cx.bind_keys` — drives both menu accelerators and
    // anywhere-in-window dispatch.
    cx.on_action(|_: &Quit, cx| confirm_quit_then_quit(cx));
    cx.on_action(|_: &About, cx| {
        dispatch_to_app_view(cx, |app, window, cx| app.shortcut_about(window, cx));
    });
    cx.on_action(|_: &OpenSettings, cx| {
        dispatch_to_app_view(cx, |app, window, cx| app.open_settings(window, cx));
    });
    cx.on_action(|_: &ToggleSidebar, cx| {
        dispatch_to_app_view(cx, |app, _window, cx| app.toggle_sidebar(cx));
    });
    cx.on_action(|action: &ConnectToProfile, cx| {
        let profile_id = action.profile_id.clone();
        dispatch_to_app_view(cx, move |app, _window, cx| {
            app.connect_profile_in_new_window(profile_id, None, cx);
        });
    });

    // App-level handlers for every shortcut action. There are two
    // problems an App-level handler solves at once on macOS:
    //
    //  1. Menu validation: AppKit asks `is_action_available` before
    //     opening a menu to decide which items to enable. gpui only
    //     reports an action as available if a handler exists either
    //     globally (here) or along the focused element's dispatch
    //     path. A handler attached to AppView's outer div is below
    //     the dispatch-tree root, so when nothing is focused (which
    //     is briefly true while the menubar is open), validation
    //     can't see it and AppKit greys the menu item out.
    //
    //  2. Click dispatch: when the user clicks a menu item, gpui
    //     dispatches from the currently-focused element. Same focus-
    //     drift problem — the per-window `.on_action` handlers on
    //     AppView's div never see the action.
    //
    // The fix is to do the actual work here, in global handlers
    // that grab the active window's root Root, downcast its inner
    // `view` to `Entity<AppView>`, and call the AppView's
    // `shortcut_*` method directly. The per-window handlers in
    // `AppView::render` still exist for the case where the action
    // is dispatched from inside the AppView focus tree (e.g. a
    // keystroke caught while the terminal is focused) — both paths
    // converge on the same `shortcut_*` method.
    cx.on_action(|_: &Connect, cx| {
        dispatch_to_app_view(cx, |app, window, cx| app.shortcut_connect(window, cx));
    });
    cx.on_action(|_: &Disconnect, cx| {
        dispatch_to_app_view(cx, |app, window, cx| app.disconnect_current(window, cx));
    });
    cx.on_action(|_: &Suspend, cx| {
        dispatch_to_app_view(cx, |app, window, cx| app.suspend_session(window, cx));
    });
    cx.on_action(|_: &Resume, cx| {
        dispatch_to_app_view(cx, |app, window, cx| app.resume_session(window, cx));
    });
    cx.on_action(|_: &ClearTerminal, cx| {
        dispatch_to_app_view(cx, |app, _window, cx| app.shortcut_clear_terminal(cx));
    });
    cx.on_action(|_: &SendBreak, cx| {
        dispatch_to_app_view(cx, |app, window, cx| app.send_break_now(window, cx));
    });
    cx.on_action(|_: &SendFile, cx| {
        dispatch_to_app_view(cx, |app, window, cx| app.start_send_file(window, cx));
    });
    cx.on_action(|_: &NewProfile, cx| {
        dispatch_to_app_view(cx, |app, window, cx| app.shortcut_new_profile(window, cx));
    });
    cx.on_action(|_: &OpenInNewWindow, cx| {
        dispatch_to_app_view(cx, |app, window, cx| {
            app.shortcut_open_in_new_window(window, cx)
        });
    });
    cx.on_action(|_: &FontIncrease, cx| {
        dispatch_to_app_view(cx, |app, _window, cx| app.shortcut_bump_font_increase(cx));
    });
    cx.on_action(|_: &FontDecrease, cx| {
        dispatch_to_app_view(cx, |app, _window, cx| app.shortcut_bump_font_decrease(cx));
    });
    cx.on_action(|_: &FontReset, cx| {
        dispatch_to_app_view(cx, |app, _window, cx| app.shortcut_bump_font_reset(cx));
    });
    // Terminal-pane clipboard actions. KeyBindings for these live
    // with `Some("Terminal")` context (see `apply_shortcut_bindings`)
    // so the keystroke only matches when the terminal div is in
    // the focus chain — typing Cmd+C in a profile-form input falls
    // through to gpui-component's Input Copy binding instead.
    // The action handlers themselves are global; once the keystroke
    // dispatches the action, route into the active window's
    // TerminalView through the existing app-view bridge.
    cx.on_action(|_: &TerminalCopy, cx| {
        dispatch_to_app_view(cx, |app, _window, cx| app.shortcut_terminal_copy(cx));
    });
    cx.on_action(|_: &TerminalPaste, cx| {
        dispatch_to_app_view(cx, |app, window, cx| {
            app.shortcut_terminal_paste(window, cx)
        });
    });
    cx.on_action(|_: &TerminalSelectAll, cx| {
        dispatch_to_app_view(cx, |app, _window, cx| app.shortcut_terminal_select_all(cx));
    });

    {
        let profile_store = profile_store.clone();
        let settings_bus = settings_bus.clone();
        let skins_store = skins_store.clone();
        let highlight_store = highlight_store.clone();
        let themes_store = themes_store.clone();
        cx.on_action(move |_: &NewWindow, cx| {
            let scrollback = settings_bus.read(cx).current().effective_scrollback();
            let terminal = cx.new(|cx| {
                TerminalView::new(24, 80, term_bridge::Palette::baudrun(), scrollback, cx)
            });
            if let Err(err) = open_app_window(
                cx,
                WindowInit::Fresh(terminal),
                None,
                profile_store.clone(),
                settings_bus.clone(),
                skins_store.clone(),
                highlight_store.clone(),
                themes_store.clone(),
            ) {
                log::error!("menu: new window: {err}");
            }
        });
    }

    // First-time bind + menu install using the boot-time settings
    // snapshot. After this the SettingsBus subscription below
    // re-runs `apply_shortcut_bindings` on every settings write so
    // the menubar accelerators stay in sync with the user's
    // overrides. The snapshot is cloned out of the bus before the
    // mutable cx borrow because `bus.read(cx)` itself needs cx.
    let boot_settings = settings_bus.read(cx).current().clone();
    apply_shortcut_bindings(cx, &boot_settings);

    // Re-bind when the user edits their shortcuts in Settings.
    // Detached so the subscription lives for the full app lifetime;
    // SettingsBus is App-scoped so there's no entity to outlive.
    cx.subscribe(&settings_bus, |_bus, event, cx| {
        // SettingsEvent::Updated carries the post-save snapshot so
        // we don't need to read the bus back (which would conflict
        // with the mutable cx borrow `apply_shortcut_bindings`
        // needs).
        let SettingsEvent::Updated(settings) = event;
        apply_shortcut_bindings(cx, settings);
    })
    .detach();
}

/// Find the active window's `AppView` entity and run `f` against
/// it. Used by the App-level shortcut handlers so menu clicks (and
/// keystrokes that arrive while focus has drifted off the AppView
/// subtree) still reach the right method.
///
/// Window root is a `gpui_component::Root`; its inner `view`
/// `AnyView` is the `Entity<AppView>` we built in `open_app_window`.
/// The downcast can fail in principle (someone else's window
/// type), but in practice every window we open is rooted in
/// AppView — a missed downcast logs and no-ops so a stray action
/// can't bring the app down.
fn dispatch_to_app_view<F>(cx: &mut App, f: F)
where
    F: FnOnce(&mut AppView, &mut Window, &mut Context<AppView>) + 'static,
{
    // Defer so the window update doesn't re-enter the same window
    // we're already inside. The menu click dispatch chain enters
    // `active_window.update(...)` to fire global handlers; trying
    // to call `handle.update(cx, ...)` on that same window from
    // within the handler hits gpui's "window not found" because
    // the window is currently `.take()`d out of the windows map.
    // `cx.defer` queues us until the outer update completes and
    // the window is back in the map.
    cx.defer(move |cx| dispatch_to_app_view_now(cx, f));
}

fn dispatch_to_app_view_now<F>(cx: &mut App, f: F)
where
    F: FnOnce(&mut AppView, &mut Window, &mut Context<AppView>) + 'static,
{
    // Prefer the platform's idea of the active window — picks the
    // right one when multiple AppView windows are open. That handle
    // can be stale (e.g. when the macOS menubar has focus the
    // returned handle fails to look up against gpui's window
    // table); fall back to scanning `cx.windows()`, which is gpui's
    // authoritative list of live windows. Order is insertion, not
    // z-order, but a stale active_window mostly happens during the
    // menu-open window where active_window won't dispatch correctly
    // anyway — the deferred dispatch runs after the menu closes
    // and the window is active again.
    let mut candidates: Vec<gpui::AnyWindowHandle> = Vec::with_capacity(4);
    if let Some(active) = cx.active_window() {
        candidates.push(active);
    }
    for handle in cx.windows() {
        if !candidates.contains(&handle) {
            candidates.push(handle);
        }
    }
    let f_cell = std::cell::RefCell::new(Some(f));
    for handle in candidates {
        let did_dispatch = std::rc::Rc::new(std::cell::Cell::new(false));
        let did_dispatch_clone = did_dispatch.clone();
        let result = handle.update(cx, |_root, window, cx| {
            let Some(Some(root)) = window.root::<Root>() else {
                return;
            };
            let view = root.read(cx).view().clone();
            let Ok(app_view) = view.downcast::<AppView>() else {
                return;
            };
            if let Some(f) = f_cell.borrow_mut().take() {
                app_view.update(cx, |app, cx| f(app, window, cx));
                did_dispatch_clone.set(true);
            }
        });
        if result.is_err() {
            continue;
        }
        if did_dispatch.get() {
            return;
        }
    }
}

/// Compute the effective binding for each shortcut action from the
/// current Settings, register all keybindings fresh, and rebuild
/// the menubar. Called at boot (with the snapshot from disk) and
/// from the SettingsBus subscription (with the new snapshot after
/// every Settings save).
///
/// We deliberately don't call `cx.clear_key_bindings()` here even
/// though it'd give us the cleanest "drop the old shortcut combo
/// when the user reassigns it" semantics. The trade-off was
/// discovered after the Windows build smoke-test: clearing nukes
/// gpui-component's Input-context keybindings (`backspace`,
/// `delete`, arrow keys, undo/redo, paste, …) that
/// `gpui_component::init` installs at boot, leaving every Input
/// widget unable to handle plain text editing. Re-installing them
/// ourselves means hand-rolling 30+ bindings across half a dozen
/// gpui-component modules and keeping that list in sync with
/// every gpui-component bump — fragile.
///
/// Trade-off accepted: a user who reassigns the same chord
/// (e.g. Cmd+K Clear → Cmd+J Clear) gets BOTH the old and new
/// chord firing the action — Cmd+K still works because the stale
/// binding is still in the keymap. The first time the user
/// reassigns the SAME chord to a different action, gpui's keymap
/// lookup picks the most-recently-added binding, so the new
/// action fires and the old doesn't. The known sharp edge: an
/// orphaned binding (old chord → action that was later moved) is
/// still active. Probably worth a "shortcut snapshot at boot,
/// emit NoAction unbinds on diff" follow-up if it actually
/// surfaces.
fn apply_shortcut_bindings(cx: &mut App, settings: &data::settings::Settings) {
    let overrides = settings.shortcuts.clone().unwrap_or_default();
    // Static accelerators that don't appear in Settings → Shortcuts.
    // Quit / New Window / Settings are always-on system bindings —
    // exposing them to the override UI would let a user accidentally
    // trap themselves in a window with no way to reach Quit or
    // Settings.
    //
    // Use `secondary-` (gpui's portable Cmd-on-macOS /
    // Ctrl-on-Windows-and-Linux token) rather than `cmd-`. gpui's
    // `cmd` / `super` / `win` literals all set the same
    // `modifiers.platform` bit, which is the Cmd key on macOS but
    // the Windows / Super key on the other two — neither of which
    // can be used as an app shortcut (the OS intercepts them
    // before they reach the window). `secondary-` is the only
    // token that fires Cmd+Q on macOS and Ctrl+Q on Windows /
    // Linux.
    let mut bindings = vec![
        KeyBinding::new("secondary-q", Quit, None),
        KeyBinding::new("secondary-n", NewWindow, None),
        KeyBinding::new("secondary-,", OpenSettings, None),
        // Standard sidebar-toggle binding (Cmd+B on macOS, Ctrl+B
        // on Windows/Linux). Same key VS Code, Sublime, Atom, and
        // every other editor with a togglable sidebar uses, so
        // muscle memory transfers in.
        KeyBinding::new("secondary-b", ToggleSidebar, None),
    ];
    // Terminal-pane clipboard actions used to live as hardcoded
    // rows here, but they now flow through Settings → Shortcuts
    // like every other customisable binding — see the
    // `"copy"` / `"paste"` / `"select-all"` ids in
    // `settings_view::SHORTCUT_ACTIONS` and the matching defaults
    // (Meta+C / Meta+V / Meta+A on macOS, Control+C / Control+V /
    // Control+A on Windows / Linux). Users who prefer the GNOME /
    // KDE convention can change their Copy spec to Control+Shift+C
    // (and free Ctrl+C back to the wire's 0x03 / ETX) from the
    // Shortcuts pane.
    // Walk the same action list Settings → Shortcuts renders so the
    // menubar and the customization UI agree on which IDs are
    // bindable. Action IDs map 1:1 to the gpui Action structs by
    // hand; an unknown ID is logged and skipped (means someone
    // added an entry to `SHORTCUT_ACTIONS` without wiring the
    // gpui side yet).
    for &id in settings_view::SHORTCUT_ACTIONS {
        let spec = settings_view::effective_shortcut(id, &overrides);
        let Some(gpui_spec) = settings_view::spec_to_gpui_binding(&spec) else {
            continue;
        };
        if let Some(b) = key_binding_for_action(id, &gpui_spec) {
            bindings.push(b);
        }
    }
    cx.bind_keys(bindings);

    install_menus(cx);
}

/// Map a Settings shortcut id (`"clear"`, `"font-increase"`, …) to
/// the corresponding gpui KeyBinding for the supplied binding
/// string. Returns `None` for unknown ids (logs once at warn level
/// so the dropped ID is debuggable without spamming on every
/// rebind).
fn key_binding_for_action(id: &str, gpui_spec: &str) -> Option<KeyBinding> {
    let kb = match id {
        "connect" => KeyBinding::new(gpui_spec, Connect, None),
        "disconnect" => KeyBinding::new(gpui_spec, Disconnect, None),
        "suspend" => KeyBinding::new(gpui_spec, Suspend, None),
        "resume" => KeyBinding::new(gpui_spec, Resume, None),
        "clear" => KeyBinding::new(gpui_spec, ClearTerminal, None),
        "break" => KeyBinding::new(gpui_spec, SendBreak, None),
        "send-file" => KeyBinding::new(gpui_spec, SendFile, None),
        "new-profile" => KeyBinding::new(gpui_spec, NewProfile, None),
        "open-window" => KeyBinding::new(gpui_spec, OpenInNewWindow, None),
        "font-increase" => KeyBinding::new(gpui_spec, FontIncrease, None),
        "font-decrease" => KeyBinding::new(gpui_spec, FontDecrease, None),
        "font-reset" => KeyBinding::new(gpui_spec, FontReset, None),
        // Terminal-pane clipboard actions are context-scoped to
        // the `Terminal` key_context the terminal div sets on
        // itself (see `TerminalView::render`). That keeps Cmd+C
        // inside a profile-form input firing gpui-component's
        // Input::Copy binding instead of grabbing the (likely
        // empty) terminal selection.
        "copy" => KeyBinding::new(gpui_spec, TerminalCopy, Some("Terminal")),
        "paste" => KeyBinding::new(gpui_spec, TerminalPaste, Some("Terminal")),
        "select-all" => KeyBinding::new(gpui_spec, TerminalSelectAll, Some("Terminal")),
        unknown => {
            log::warn!("shortcut: unknown action id {unknown}");
            return None;
        }
    };
    Some(kb)
}

/// Install the macOS NSMenuBar (and no-op on other platforms). The
/// accelerators next to each label come from whatever bindings are
/// currently registered for the action types via `cx.bind_keys`,
/// so this needs to be called AFTER `bind_keys` in the rebind path.
fn install_menus(cx: &mut App) {
    cx.set_menus(vec![
        // The first menu's name is overridden by the bundle's
        // display name on macOS, so "Baudrun" here is mostly for
        // hint purposes. Services / Hide / Hide Others / Show All
        // are added automatically by AppKit when the platform
        // recognises the slot.
        Menu::new("Baudrun").items([
            MenuItem::action("About Baudrun", About),
            MenuItem::separator(),
            MenuItem::action("Settings…", OpenSettings),
            MenuItem::separator(),
            MenuItem::action("Quit Baudrun", Quit),
        ]),
        Menu::new("File").items([
            MenuItem::action("New Window", NewWindow),
            MenuItem::action("New Profile", NewProfile),
            MenuItem::separator(),
            MenuItem::action("Send File…", SendFile),
            MenuItem::action("Open Profile in New Window", OpenInNewWindow),
        ]),
        // Standard macOS Edit menu — accelerator labels come from
        // the registered KeyBindings (Cmd+C / Cmd+V / Cmd+A in
        // `apply_shortcut_bindings`). The actions themselves are
        // context-scoped to "Terminal", so picking them from the
        // menu while no terminal is in focus is a no-op rather
        // than a crash — gpui's keymap dispatcher refuses to
        // route the action when the active focus chain doesn't
        // carry the matching context.
        Menu::new("Edit").items([
            MenuItem::action("Copy", TerminalCopy),
            MenuItem::action("Paste", TerminalPaste),
            MenuItem::separator(),
            MenuItem::action("Select All", TerminalSelectAll),
        ]),
        Menu::new("Session").items([
            MenuItem::action("Connect", Connect),
            MenuItem::action("Disconnect", Disconnect),
            MenuItem::separator(),
            MenuItem::action("Suspend Session", Suspend),
            MenuItem::action("Resume Session", Resume),
            MenuItem::separator(),
            MenuItem::action("Send Break", SendBreak),
        ]),
        Menu::new("View").items([
            MenuItem::action("Clear Terminal", ClearTerminal),
            MenuItem::separator(),
            MenuItem::action("Increase Font Size", FontIncrease),
            MenuItem::action("Decrease Font Size", FontDecrease),
            MenuItem::action("Reset Font Size", FontReset),
        ]),
        Menu::new("Window"),
        Menu::new("Help"),
    ]);
}

/// Dev-mode dock-icon override for macOS. Loads
/// `resources/icons/icon.icns` and re-renders it onto a 1024×1024
/// canvas with a ~10% transparent margin on all sides before
/// handing it to `NSApplication.applicationIconImage`. The margin
/// matches Apple's macOS Big Sur+ icon "live area" guideline
/// (icon artwork should occupy roughly the inner 80% of the
/// canvas, with the surrounding pixels transparent so the dock
/// can size every app's icon consistently). The source `.icns`
/// was generated by Tauri without that safe-area inset baked in,
/// so without the runtime re-canvas the icon appears oversized
/// next to other dock entries. No-op when the file isn't
/// reachable; production builds get the icon from the .app
/// bundle's Info.plist + Resources/icon.icns where the bundling
/// pipeline applies its own scaling.
#[cfg(target_os = "macos")]
fn install_macos_dock_icon() {
    use objc2::AnyThread;
    use objc2_app_kit::{NSApplication, NSCompositingOperation, NSImage};
    use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};
    const CANVAS_PX: f64 = 1024.0;
    const CONTENT_PX: f64 = 824.0; // ~80% of canvas — Apple's "live area"
    const INSET_PX: f64 = (CANVAS_PX - CONTENT_PX) / 2.0;

    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let icon_path = format!("{manifest_dir}/resources/icons/icon.icns");
    // SAFETY: We're on the main thread (gpui_platform::application
    // runs its main callback on the main thread on macOS). The
    // ObjC objects we touch (NSApplication.sharedApplication,
    // NSImage, NSGraphicsContext via lockFocus) are all main-
    // thread-only types.
    //
    // lockFocus / unlockFocus are deprecated in favour of the
    // resolution-independent block-based
    // `imageWithSize:flipped:drawingHandler:` API, but the
    // block-callback path adds a closure-boxing dance for
    // marginal benefit on a one-shot dock-icon override that
    // never re-renders. Keep the simpler form with the deprecation
    // suppressed; revisit if the icon ever needs to redraw on a
    // display-mode change.
    #[allow(deprecated)]
    unsafe {
        let path = NSString::from_str(&icon_path);
        let Some(source) = NSImage::initWithContentsOfFile(NSImage::alloc(), &path) else {
            log::warn!("dock icon: could not load {icon_path}");
            return;
        };
        let canvas = NSImage::initWithSize(NSImage::alloc(), NSSize::new(CANVAS_PX, CANVAS_PX));
        canvas.lockFocus();
        // Empty fromRect tells Cocoa to use the source image's
        // natural extent. Operation=copy means we paint the
        // source pixels directly (no blend with the transparent
        // canvas underneath); fraction=1.0 = full opacity.
        source.drawInRect_fromRect_operation_fraction(
            NSRect::new(
                NSPoint::new(INSET_PX, INSET_PX),
                NSSize::new(CONTENT_PX, CONTENT_PX),
            ),
            NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(0.0, 0.0)),
            NSCompositingOperation::Copy,
            1.0,
        );
        canvas.unlockFocus();

        let mtm = objc2::MainThreadMarker::new_unchecked();
        let app = NSApplication::sharedApplication(mtm);
        app.setApplicationIconImage(Some(&canvas));
    }
}

/// Query the OS "reduce motion" accessibility preference. Wired
/// to the `ReduceMotion` global at boot. macOS reads
/// `NSWorkspace.accessibilityDisplayShouldReduceMotion`; Windows
/// reads `SystemParametersInfo(SPI_GETCLIENTAREAANIMATION)`; Linux
/// returns false until a GTK / freedesktop-portal query is wired
/// (low priority — gpui's Linux story is least mature anyway).
#[cfg(target_os = "macos")]
fn detect_reduce_motion() -> bool {
    use objc2_app_kit::NSWorkspace;
    // NSWorkspace.sharedWorkspace + the accessibility-display
    // accessor on the resulting object are both safe in objc2's
    // bindings (no `unsafe fn` markers); no unsafe block needed.
    NSWorkspace::sharedWorkspace().accessibilityDisplayShouldReduceMotion()
}

#[cfg(target_os = "windows")]
fn detect_reduce_motion() -> bool {
    // `SystemParametersInfoW(SPI_GETCLIENTAREAANIMATION)` writes a
    // Win32 `BOOL` (`i32` in Rust FFI terms) into the supplied
    // buffer: nonzero = client-area animations are ON, zero = the
    // user has flipped the "Show animations in Windows" toggle off
    // (Settings → Accessibility → Visual effects → Animation
    // effects). Reduce-motion is therefore the inverse of the
    // returned value.
    //
    // Inline FFI rather than pulling `windows-sys` in as a direct
    // dep for one constant + one function — gpui_windows already
    // brings windows-sys transitively but pinning a direct version
    // would risk conflicts on every gpui bump. `i32` used directly
    // instead of a `BOOL` typedef so clippy's
    // `upper_case_acronyms` lint stays happy on the Windows CI
    // job.
    const SPI_GETCLIENTAREAANIMATION: u32 = 0x1042;
    unsafe extern "system" {
        fn SystemParametersInfoW(
            uiAction: u32,
            uiParam: u32,
            pvParam: *mut core::ffi::c_void,
            fWinIni: u32,
        ) -> i32;
    }
    let mut enabled: i32 = 1;
    let ok = unsafe {
        SystemParametersInfoW(
            SPI_GETCLIENTAREAANIMATION,
            0,
            &mut enabled as *mut _ as *mut _,
            0,
        )
    };
    if ok == 0 {
        // Query failed — default to "animations on / reduce motion
        // off" as the safer fallback (matches every other
        // platform's no-op default).
        return false;
    }
    enabled == 0
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn detect_reduce_motion() -> bool {
    false
}

/// Pop a modal Windows error dialog with the given caption + body.
/// Used by the two startup-failure paths
/// (`show_window_open_error_dialog`, `show_dxgi_unavailable_dialog`)
/// so the wide-string encoding + `MessageBoxW` FFI live in one place.
///
/// Release builds set `windows_subsystem = "windows"` (no attached
/// console), so `log::error!` is invisible to the user — `MessageBoxW`
/// is the only feedback path that actually surfaces a startup-time
/// failure. The dialog blocks the calling thread until the user
/// clicks OK, which also keeps the process alive long enough that
/// the winget validator's launch test sees a normal "GUI app waiting
/// on user input" outcome instead of an instant crash with
/// `STATUS_STACK_BUFFER_OVERRUN`.
///
/// Inline FFI on `MessageBoxW` (user32.dll) for the same reason
/// `detect_reduce_motion` uses inline `SystemParametersInfoW`:
/// avoids a direct `windows-sys` dep that'd conflict with
/// gpui_windows's transitive version on every gpui bump.
#[cfg(target_os = "windows")]
fn show_windows_error_dialog(caption: &str, body: &str) {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;

    let to_wide = |s: &str| -> Vec<u16> {
        OsStr::new(s)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    };
    let body_w = to_wide(body);
    let caption_w = to_wide(caption);

    // MB_OK | MB_ICONERROR = "[ OK ]" button + red-X icon. No need
    // for the SETFOREGROUND flag — the app isn't on screen yet (no
    // window was opened) so the dialog comes up as the only
    // foreground window for our process.
    const MB_OK: u32 = 0x0;
    const MB_ICONERROR: u32 = 0x10;
    unsafe extern "system" {
        fn MessageBoxW(
            hwnd: *mut core::ffi::c_void,
            text: *const u16,
            caption: *const u16,
            utype: u32,
        ) -> i32;
    }
    unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            body_w.as_ptr(),
            caption_w.as_ptr(),
            MB_OK | MB_ICONERROR,
        );
    }
}

/// Pop a modal Windows error dialog when `open_app_window` fails.
/// Fires from the `Err(err)` arm of the `open_app_window` match in
/// `main`; on the same code path as `dxgi_probe`'s fallback dialog
/// (see `show_dxgi_unavailable_dialog`) but reached only when the
/// probe passed AND the actual gpui-side `open_window` still failed.
#[cfg(target_os = "windows")]
fn show_window_open_error_dialog<E: std::fmt::Debug>(err: &E) {
    // Body carries the full anyhow chain via `{err:?}` so the user
    // (or a support thread) sees the underlying HRESULT, not just
    // "couldn't open window". Wrapping copy explains the most
    // common cause we've observed (DXGI device not currently
    // available) without committing to that being the only one.
    let body = format!(
        "Baudrun couldn't initialize its window.\n\n\
         {err:?}\n\n\
         This usually means the graphics adapter isn't currently \
         available — paused driver, headless session, or GPU \
         exhaustion. Try restarting Windows, or sign into a \
         desktop session before launching Baudrun."
    );
    show_windows_error_dialog("Baudrun — failed to start", &body);
}

/// Pre-flight check for an available Direct3D-capable graphics
/// adapter, called from `main` before any gpui state exists.
///
/// Calls `D3D11CreateDevice` with `D3D_DRIVER_TYPE_HARDWARE` and
/// every `ppDevice` / `ppImmediateContext` out-arg set to null —
/// we don't actually want a device, we just want the HRESULT.
/// `S_OK` (0) and any other non-negative result means the OS can
/// hand a Direct3D device to gpui's renderer; a negative HRESULT
/// means it can't, and gpui's own `D3D11CreateDevice` call would
/// fail the same way a few hundred milliseconds later.
///
/// The HRESULT we specifically care about is
/// `DXGI_ERROR_NOT_CURRENTLY_AVAILABLE` (`0x887A0022`), but the
/// probe doesn't special-case it — any negative HRESULT bails
/// out, because there's no graceful-recover path for "gpui's
/// renderer can't initialize" regardless of the underlying reason.
///
/// Inline FFI on `D3D11CreateDevice` (d3d11.dll) matches the shape
/// of `show_windows_error_dialog`'s `MessageBoxW` FFI and
/// `detect_reduce_motion`'s `SystemParametersInfoW` FFI for the
/// same reason: avoids a direct `windows-sys` dep that'd conflict
/// with `gpui_windows`'s transitive version on every gpui bump.
#[cfg(target_os = "windows")]
fn dxgi_probe() -> Result<(), u32> {
    // D3D_DRIVER_TYPE_HARDWARE = 1 — ask for a real hardware
    // adapter (not WARP / reference / software). Matches what
    // gpui's renderer init asks for, so a probe-side success means
    // gpui's side will also find an adapter.
    const D3D_DRIVER_TYPE_HARDWARE: i32 = 1;
    // D3D11_SDK_VERSION = 7, the value the D3D11 headers have
    // declared since the SDK shipped. Passing this is a versioning
    // handshake with d3d11.dll; the constant doesn't change across
    // Windows releases.
    const D3D11_SDK_VERSION: u32 = 7;

    unsafe extern "system" {
        fn D3D11CreateDevice(
            adapter: *mut core::ffi::c_void,
            driver_type: i32,
            software: *mut core::ffi::c_void,
            flags: u32,
            feature_levels: *const i32,
            feature_levels_count: u32,
            sdk_version: u32,
            device: *mut *mut core::ffi::c_void,
            feature_level: *mut i32,
            immediate_context: *mut *mut core::ffi::c_void,
        ) -> i32;
    }

    // SAFETY: All pointer args are either null (we want d3d11.dll
    // to skip the corresponding out-write) or fixed-size primitive
    // values passed by value via the ABI. d3d11.dll ships with
    // Windows so the import is always satisfied at load time.
    let hr = unsafe {
        D3D11CreateDevice(
            std::ptr::null_mut(),
            D3D_DRIVER_TYPE_HARDWARE,
            std::ptr::null_mut(),
            0,
            std::ptr::null(),
            0,
            D3D11_SDK_VERSION,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    if hr >= 0 {
        Ok(())
    } else {
        Err(hr as u32)
    }
}

/// Pop a modal Windows error dialog explaining a `dxgi_probe`
/// failure, then return so the caller can `std::process::exit(0)`.
/// The body surfaces the raw HRESULT so a support thread can
/// correlate against the underlying Windows error.
#[cfg(target_os = "windows")]
fn show_dxgi_unavailable_dialog(hr: u32) {
    let body = format!(
        "Baudrun couldn't initialize a graphics adapter (HRESULT 0x{hr:08X}).\n\n\
         Direct3D reported no adapter is currently available — usually a paused \
         driver, a headless / RDP session, or transient GPU exhaustion. Try \
         restarting Windows, or sign into a desktop session before launching \
         Baudrun."
    );
    show_windows_error_dialog("Baudrun — failed to start", &body);
}

/// Right-click menu on the macOS dock icon (no-op on other
/// platforms — gpui's `set_dock_menu` swallows non-mac calls).
/// Shows "New Window" plus a recent-profile shortlist: clicking
/// a profile opens a fresh window already connected to that
/// profile, mirroring the sidebar's right-click "Open in New
/// Window" entry.
///
/// Built once at boot from the current profile store. Doesn't
/// live-update when the user creates / renames / deletes a
/// profile — the profile store doesn't emit change events, and
/// the dock menu is a discovery affordance rather than a precise
/// real-time UI. A relaunch refreshes it; the dock UX expectation
/// is "menu shows what was true when the app started," same
/// pattern as Recent Files on most other macOS apps.
///
/// The profile list is capped at `MAX_DOCK_PROFILES` so the
/// right-click menu doesn't sprout 50 rows for a user with a
/// large profile collection — anything past the cap stays
/// reachable through the sidebar.
///
/// cfg-gated to macOS only — the call site already is, and
/// without this gate Windows / Linux clippy flags the function
/// as dead code (it's never reachable on those platforms).
#[cfg(target_os = "macos")]
fn install_dock_menu(cx: &App, profile_store: &Rc<data::profiles::Store>) {
    const MAX_DOCK_PROFILES: usize = 10;
    let mut items = vec![MenuItem::action("New Window", NewWindow)];
    // Profile order matches `Store::list` (creation order on
    // disk). Picking by stable order means the same dock click
    // always opens the same profile session-to-session, even when
    // the user is mid-rename — a frequency-sorted list would
    // jitter and confuse muscle memory.
    let profiles = profile_store.list();
    if !profiles.is_empty() {
        items.push(MenuItem::separator());
        for profile in profiles.into_iter().take(MAX_DOCK_PROFILES) {
            items.push(MenuItem::action(
                profile.name.clone(),
                ConnectToProfile {
                    profile_id: profile.id.clone(),
                },
            ));
        }
    }
    cx.set_dock_menu(items);
}

/// App-scoped handles installed as a gpui `Global` so any handler
/// that runs outside the original `run` closure (notably
/// `on_reopen`, which is registered on `Application` before `run`
/// starts) can grab the live store + bus references. Everything in
/// here is also cloned into the menubar / dock-menu / per-window
/// paths during boot; this struct is the canonical "what does the
/// app need to spawn another window" bundle.
struct AppShared {
    profile_store: Rc<data::profiles::Store>,
    settings_bus: gpui::Entity<SettingsBus>,
    skins_store: Rc<data::skins::Store>,
    highlight_store: Rc<data::highlight::Store>,
    themes_store: Rc<data::themes::Store>,
}
impl gpui::Global for AppShared {}

/// macOS single-instance support. When `LSMultipleInstancesProhibited`
/// in Info.plist is set (it is), Launch Services routes any second
/// launch attempt — double-click on the .app, `open -a Baudrun`,
/// double-click on a `.baudrun-profile.json` — to the existing
/// process instead of spawning a duplicate. Inside the existing
/// process the NSApplication delegate fires
/// `applicationShouldHandleReopen:`, which gpui surfaces here as
/// `Application::on_reopen`.
///
/// The handler activates the app (brings it to front) and, when
/// no windows are visible, spawns a fresh welcome window. That
/// matches the macOS convention: clicking the dock icon on a
/// running app with no visible windows opens a new one (Finder,
/// Safari, Mail all behave this way).
///
/// Other platforms: Windows / Linux don't have Launch Services'
/// equivalent free of charge; cross-platform single-instance
/// would need a named-mutex (Windows) or unix-domain-socket
/// (Linux) handshake. The Info.plist key only covers macOS for
/// now.
fn handle_reopen(cx: &mut App) {
    // Activating with `ignoring_other_apps = true` is the standard
    // "give the user focus right now" call. Without it the app
    // stays backgrounded and the dock click looks like a no-op.
    cx.activate(true);
    // No live windows: open a fresh welcome window. `cx.windows()`
    // is gpui's authoritative list of live entries — more
    // reliable than `cx.active_window()`, which can point at the
    // menubar's pseudo-window even when no real windows exist.
    if !cx.windows().is_empty() {
        return;
    }
    // `set_global` ran at the end of boot, so if reopen fires
    // before `run`'s closure finishes setting things up we'd miss
    // it; in practice macOS doesn't fire reopen until after the
    // app finishes launching, but guard anyway.
    if !cx.has_global::<AppShared>() {
        log::warn!("reopen: AppShared not initialised yet");
        return;
    }
    let shared = cx.global::<AppShared>();
    let scrollback = shared
        .settings_bus
        .read(cx)
        .current()
        .effective_scrollback();
    let profile_store = shared.profile_store.clone();
    let settings_bus = shared.settings_bus.clone();
    let skins_store = shared.skins_store.clone();
    let highlight_store = shared.highlight_store.clone();
    let themes_store = shared.themes_store.clone();
    let terminal =
        cx.new(|cx| TerminalView::new(24, 80, term_bridge::Palette::baudrun(), scrollback, cx));
    if let Err(err) = open_app_window(
        cx,
        WindowInit::Fresh(terminal),
        None,
        profile_store,
        settings_bus,
        skins_store,
        highlight_store,
        themes_store,
    ) {
        log::error!("reopen: spawn welcome window: {err}");
    }
}

/// Quit gate: if any window has a live serial session, show a
/// confirmation dialog in that window before tearing the whole
/// app down. Otherwise quit immediately. Wired to the `Quit`
/// action (Cmd+Q in the menubar) so stray keystrokes don't lose
/// an active connection or an in-flight X/YMODEM transfer.
///
/// Counts these as "live":
///   * a connected profile (real bytes on the wire)
///   * an in-flight X/YMODEM transfer (cancelling tears down
///     the send / receive task immediately)
///   * an active auto-reconnect retry loop — the user explicitly
///     left a profile selected expecting Baudrun to reattach
///     when the port comes back.
///
/// "Welcome screen" and "editor open with nothing connected"
/// don't count: closing the app from there doesn't lose state
/// the user would care about — the profile store is already
/// persisted to disk.
fn confirm_quit_then_quit(cx: &mut App) {
    // Defer out of the current window-update so the per-window
    // `handle.update` calls below don't try to re-enter the same
    // window we're already inside (same gotcha as
    // `dispatch_to_app_view`: when the Quit action is dispatched
    // from the menubar, gpui wraps the global handler in
    // `active_window.update(...)`, and a second update for the
    // same handle hits "window not found").
    cx.defer(confirm_quit_then_quit_inner);
}

fn confirm_quit_then_quit_inner(cx: &mut App) {
    // Find the first window with a live session. We don't need
    // to enumerate them all — the dialog goes in front of one
    // window, and quit affects every window equally.
    let live_window: Option<gpui::AnyWindowHandle> = cx.windows().into_iter().find(|handle| {
        handle
            .update(cx, |_root, window, cx| {
                let Some(Some(root)) = window.root::<Root>() else {
                    return false;
                };
                let Ok(app_view) = root.read(cx).view().clone().downcast::<AppView>() else {
                    return false;
                };
                app_view.read(cx).has_live_session()
            })
            .unwrap_or(false)
    });
    let Some(handle) = live_window else {
        cx.quit();
        return;
    };
    // Dialog in the window that owns the live session, so the
    // confirmation is anchored to the work that's about to be
    // lost. `open_alert_dialog` plus `show_cancel(true)` gives
    // the canonical macOS "Quit / Cancel" pair. `on_ok` fires
    // when the user clicks Quit; that's where we actually call
    // `cx.quit()`.
    let _ = handle.update(cx, |_root, window, cx| {
        window.open_alert_dialog(cx, |alert, _, _| {
            // `show_cancel` writes through to `button_props.show_cancel`,
            // and `button_props(...)` replaces the whole struct — so
            // call show_cancel ON DialogButtonProps directly to avoid
            // the chain order silently dropping it. `keyboard(true)`
            // is already the AlertDialog default but we set it
            // explicitly so the contract is visible in source.
            alert
                .title("Quit Baudrun?")
                .description(
                    "A serial session is still active. Quitting will \
                     disconnect it and cancel any in-flight transfers.",
                )
                .button_props(
                    gpui_component::dialog::DialogButtonProps::default()
                        .show_cancel(true)
                        .ok_text("Quit")
                        .cancel_text("Cancel"),
                )
                .keyboard(true)
                .on_ok(|_, _window, cx| {
                    cx.quit();
                    true
                })
        });
    });
}

fn fallback_profile_store(reason: String) -> Rc<data::profiles::Store> {
    eprintln!("{reason}; using empty in-tmpdir profile store");
    let tmp = std::env::temp_dir().join("baudrun-empty");
    Rc::new(data::profiles::Store::new(&tmp).expect("temp profile store should always init"))
}

/// Settings-store equivalent of `fallback_profile_store`: lets the
/// app open with the built-in `Settings::default()` when we can't
/// touch the real config dir (read-only home, missing perms…).
/// Edits made via the UI in this state still write to the tmpdir
/// path and are lost between launches — that's the trade for not
/// crashing.
fn fallback_settings_store(reason: String) -> Rc<data::settings::Store> {
    eprintln!("{reason}; using default in-tmpdir settings store");
    let tmp = std::env::temp_dir().join("baudrun-empty");
    Rc::new(data::settings::Store::new(&tmp).expect("temp settings store should always init"))
}

/// Skins-store fallback. Built-in skins are still available — they
/// embed at compile time — so the user-skin import path is the only
/// thing this fallback loses. Same trade as the other fallbacks:
/// the UI keeps working, edits round-trip to the tmpdir until the
/// real config dir comes back.
fn fallback_skins_store(reason: String) -> Rc<data::skins::Store> {
    eprintln!("{reason}; using empty in-tmpdir skins store");
    let tmp = std::env::temp_dir().join("baudrun-empty");
    Rc::new(data::skins::Store::new(&tmp).expect("temp skins store should always init"))
}

/// Highlight-pack-store fallback. Bundled packs (built-in vendor
/// rule sets) embed at compile time so the picker still has rows
/// to render; only the user pack + custom imports are lost.
fn fallback_highlight_store(reason: String) -> Rc<data::highlight::Store> {
    eprintln!("{reason}; using empty in-tmpdir highlight store");
    let tmp = std::env::temp_dir().join("baudrun-empty");
    Rc::new(data::highlight::Store::new(&tmp).expect("temp highlight store should always init"))
}

/// Themes-store fallback. Built-in themes embed at compile time so
/// the picker is still populated; only user-imported `.itermcolors`
/// / JSON themes are lost.
fn fallback_themes_store(reason: String) -> Rc<data::themes::Store> {
    eprintln!("{reason}; using empty in-tmpdir themes store");
    let tmp = std::env::temp_dir().join("baudrun-empty");
    Rc::new(data::themes::Store::new(&tmp).expect("temp themes store should always init"))
}
