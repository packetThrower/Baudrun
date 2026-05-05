//! Window chrome + multi-window commands.
//!
//! - `set_traffic_lights_inset`: macOS-specific traffic-light
//!   repositioning via `tauri-plugin-decorum` so skins with a
//!   floating-bubble layout (macos-26 Liquid Glass) can pull the
//!   lights inside the panel. Non-macOS platforms accept the call
//!   and no-op.
//! - `open_profile_window`: spawn a new top-level webview pointing
//!   at the same renderer (plain `index.html`). The initial profile
//!   id is stashed in `AppState::pending_profile_ids` keyed by the
//!   new window's label so the renderer can drain it on mount via
//!   `take_pending_profile_id`. The new window has its own session
//!   in [`crate::state::AppState`] keyed by its label, so
//!   connecting / disconnecting / transferring on one window
//!   doesn't disturb the others.
//! - `toggle_settings_window`: open or close the dedicated Settings
//!   window. Singleton — second call while one is open closes it
//!   (per the v0.9.5 shape brief: clicking the sidebar Settings
//!   button while the window is open closes it). The window's label
//!   is the literal string `"settings"`, distinguishing it from the
//!   `"main"` window and the `win-<uuid>` profile-spawn windows.
//!   Frontend routes view by reading
//!   `getCurrentWebviewWindow().label`.

use std::sync::Arc;

use tauri::{
    AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
};
use uuid::Uuid;

use crate::events;
use crate::state::AppState;

/// Reduce a profile id to a URL-safe slice. Called on every command
/// boundary that accepts a profile id from the renderer so values
/// can't smuggle path / quote / scheme characters across the IPC
/// surface.
fn safe_profile_id(profile_id: &str) -> String {
    profile_id
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .collect()
}

/// Pick the OS-level webview background color a freshly-spawned
/// window should use. WebView2 and WebKitGTK paint their own default
/// (black on Windows, white-ish on Linux) during the brief gap
/// between window construction and the first HTML paint. Setting
/// this on the builder hides that flash by giving the OS something
/// reasonable to draw before any HTML has loaded.
///
/// Maps the user's saved `appearance` preference: "light" → light
/// gray, anything else (including "auto" and "dark") → dark gray.
/// "auto" defaults to dark because (a) most network-engineer users
/// run a dark UI and (b) detecting system theme from Rust at this
/// pre-window-creation point isn't reliable across all platforms.
/// Even when the chosen color doesn't perfectly match the eventual
/// skin, a brief mismatched flash is far less jarring than the raw
/// black flash users were reporting.
fn background_color_for_appearance(appearance: &str) -> tauri::window::Color {
    match appearance {
        // #f2f2f7 — matches the macOS-26 light skin's --shell-bg
        // midpoint so the transition into Liquid Glass is seamless.
        "light" => tauri::window::Color(242, 242, 247, 255),
        // #1d1d1d — sits near the lower edge of every dark skin's
        // shell gradient (macOS-26, windows-11, default). Close
        // enough that the transition isn't perceptible on most
        // skins.
        _ => tauri::window::Color(29, 29, 29, 255),
    }
}

#[tauri::command]
pub fn set_traffic_lights_inset(
    x: f32,
    y: f32,
    window: WebviewWindow,
) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        use tauri_plugin_decorum::WebviewWindowExt;
        window
            .set_traffic_lights_inset(x, y)
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (x, y, window);
        Ok(())
    }
}

#[tauri::command]
pub fn open_profile_window(
    profile_id: String,
    profile_name: Option<String>,
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<String, String> {
    // Perf instrumentation (alpha track) — same `[perf]` tag as the
    // Settings window path so both code paths grep into one timeline.
    let ipc_enter = std::time::Instant::now();
    log::info!("[perf] profile-window: ipc-handler-enter");
    let safe_id = safe_profile_id(&profile_id);
    if safe_id.is_empty() {
        return Err("profile id required".into());
    }

    // Each spawned window gets a unique label so per-window session
    // state in AppState keys cleanly.
    let label = format!("win-{}", Uuid::new_v4().simple());
    let title = match profile_name {
        Some(name) if !name.trim().is_empty() => format!("Baudrun — {}", name.trim()),
        _ => "Baudrun".to_string(),
    };

    // Stash the initial profile id under the new window's label so
    // its renderer can pull it on mount via `take_pending_profile_id`
    // and pre-select that profile. The previous design rode this in
    // the URL as `?profile=<id>`, but `?` is an invalid path char on
    // Windows — `WebviewUrl::App` builds its URL from a `PathBuf` and
    // the `?` either got stripped or url-encoded, leading to a 404
    // and a blank webview on Windows spawned windows. IPC sidesteps
    // platform path semantics entirely.
    state.store_pending_profile_id(&label, safe_id.clone());

    // Snapshot the appearance preference now (before crossing the
    // tokio boundary) so the spawned task can pick a sensible OS-
    // level webview bg. Without this, the new window starts with a
    // black surface that's visible until the JS bundle paints.
    let bg_color = background_color_for_appearance(&state.settings.get().appearance);

    // Build the window OFF the IPC dispatcher thread.
    //
    // alpha.1 / alpha.2 still wedged on Windows even after dropping
    // set_focus(), so the deadlock isn't just that one call — it's
    // that `WebviewWindowBuilder::build()` itself runs synchronously
    // inside the IPC handler. While that handler is in flight,
    // Tauri 2's IPC dispatcher and Windows-side WebView2 protocol
    // handler can't service the new window's own bootstrap fetches
    // (HTML / JS / CSS over `tauri.localhost`, IPC commands like
    // take_pending_profile_id over `ipc.localhost`). Net result:
    // the new window is created at the OS level but its renderer
    // stalls on its initial document load, looks blank, and won't
    // even respond to F12 / right-click / X (all of which need
    // a live renderer).
    //
    // tauri::async_runtime::spawn moves the build to a tokio task,
    // letting this command return Ok(label) immediately. The
    // calling renderer's await resolves; the IPC dispatcher is
    // free; Tauri 2's protocol handler can serve the new window's
    // bootstrap requests. migrate_session and the new window's own
    // mount-time IPCs (take_pending_*) all touch state that exists
    // independently of whether build() has finished, so there's no
    // race here — they read AppState directly.
    let app_clone = app.clone();
    let label_for_task = label.clone();
    tauri::async_runtime::spawn(async move {
        log::info!(
            "[perf] profile-window: task-start (after-spawn={}ms)",
            ipc_enter.elapsed().as_millis()
        );
        let build_start = std::time::Instant::now();
        let url = WebviewUrl::default();
        // `title_bar_style` and `hidden_title` only exist on the
        // macOS builder API — chaining them unconditionally breaks
        // the Windows + Linux compiles (E0599). On macOS they match
        // the main window's overlay-titlebar treatment from
        // tauri.conf.json so spawned windows look identical to
        // main; other platforms get the default decorated chrome
        // the conf file describes.
        //
        // DevTools were enabled across the 0.9.4 alpha track for
        // diagnostic visibility into the Windows blank-screen issue;
        // disabled again for stable since production users don't
        // need the WebView2 / WebKit DevTools UI exposed by default.
        // Re-enable by adding `"devtools"` to the tauri feature list
        // in Cargo.toml plus a `.devtools(true)` builder call here.
        // Show-on-ready: build invisible, let App.svelte's onMount
        // call .show() after applySkin paints. The bg flash users
        // were seeing wasn't an HTML/CSS-cascade issue (we covered
        // those with the inline #app pre-paint and Tauri builder
        // background_color). It was the OS-level webview compositing
        // an empty surface for the ~100-200ms window before any
        // content reached first paint, regardless of our
        // background_color hint. Building invisible removes that
        // gap entirely — the window only appears once content has
        // already been composited. The background_color call below
        // is kept as a safety net in case JS doesn't run for some
        // reason and a Rust-side fallback ends up showing the
        // window without the JS path completing.
        #[allow(unused_mut)]
        let mut builder = WebviewWindowBuilder::new(&app_clone, &label_for_task, url)
            .title(title)
            .inner_size(1100.0, 720.0)
            .min_inner_size(800.0, 500.0)
            .visible(false)
            .background_color(bg_color);
        #[cfg(target_os = "macos")]
        {
            builder = builder
                .title_bar_style(tauri::TitleBarStyle::Overlay)
                .hidden_title(true);
        }
        match builder.build() {
            Ok(_window) => {
                log::info!(
                    "[perf] profile-window: built (build={}ms total-since-ipc={}ms)",
                    build_start.elapsed().as_millis(),
                    ipc_enter.elapsed().as_millis()
                );
                // macOS-only chrome touch-ups via decorum. Failures
                // aren't fatal — window still opens, chrome just
                // looks default. Calling decorum on Windows strips
                // the native frame without providing a CSS
                // replacement (would hide caption buttons; issue #7).
                #[cfg(target_os = "macos")]
                {
                    use tauri_plugin_decorum::WebviewWindowExt;
                    if let Err(err) = _window.create_overlay_titlebar() {
                        log::warn!(
                            "spawned window {}: create_overlay_titlebar: {}",
                            label_for_task,
                            err
                        );
                    }
                    if let Err(err) = _window.set_traffic_lights_inset(14.0, 20.0) {
                        log::warn!(
                            "spawned window {}: set_traffic_lights_inset: {}",
                            label_for_task,
                            err
                        );
                    }
                    log::info!("[perf] profile-window: chrome-applied");
                }
                // No explicit set_focus() — newly-created windows
                // come to foreground naturally on every desktop OS,
                // and the explicit call was the original suspect for
                // the Windows hang (turned out only part of the
                // problem; this task-spawn covers the rest).
            }
            Err(err) => {
                log::error!(
                    "spawned window {} build failed: {}",
                    label_for_task,
                    err
                );
            }
        }
    });

    Ok(label)
}

/// Move the calling window's session into `target_label`'s slot. The
/// frontend calls this right after [`open_profile_window`] when the
/// dragged-out profile was the source window's active session — so
/// the new window inherits the live serial connection instead of
/// starting fresh while the source loses ownership of the port.
///
/// The shared event-target-label cell stored in the session handle
/// is updated atomically, so background threads that have read the
/// old label will fire one final emit toward the source before the
/// next one routes to the target. That tiny race is acceptable for
/// a tear-off; the source's frontend clears its session UI on
/// success and any straggler events become no-ops.
///
/// Refuses migration when:
///   - source has no active session
///   - target already has an active session (would shadow / leak)
///   - source has a transfer in flight (the transfer's progress
///     closure binds the label statically — wait or cancel first)
#[tauri::command]
pub fn migrate_session(
    target_label: String,
    terminal_snapshot: Option<String>,
    window: WebviewWindow,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    let from_label = window.label().to_string();
    // Sanitize on the way in — Tauri lets renderers send arbitrary
    // strings, and target_label is used as a HashMap key + emit_to
    // route. Same alphabet as `open_profile_window`'s id sanitizer.
    let target_label: String = target_label
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .collect();
    if target_label.is_empty() {
        return Err("target window label required".into());
    }
    if from_label == target_label {
        return Err("source and target are the same window".into());
    }

    let mut sessions = state.sessions.lock().unwrap();
    {
        let source = sessions
            .get(&from_label)
            .ok_or_else(|| "source has no session".to_string())?;
        if source.session.is_none() {
            return Err("source has no active session to migrate".into());
        }
        if source.transfer_cancel.is_some() {
            return Err(
                "transfer in progress — wait for it to finish or cancel before migrating".into(),
            );
        }
    }
    {
        // Don't accidentally clobber an in-progress connection in the
        // target — bail before disturbing the source.
        if let Some(target) = sessions.get(&target_label) {
            if target.session.is_some() {
                return Err("target already has an active session".into());
            }
        }
    }

    // Take source's contents, leaving an empty handle in its slot so
    // the entry can be removed cleanly afterwards. std::mem::take
    // gives back a default SessionHandle with all fields cleared.
    let moved = sessions
        .get_mut(&from_label)
        .map(std::mem::take)
        .unwrap_or_default();

    // Update the shared cell so the read-pump's on_read / on_exit
    // closures emit to the target window from now on.
    if let Some(cell) = &moved.event_target_label {
        if let Ok(mut guard) = cell.write() {
            *guard = target_label.clone();
        }
    }

    sessions.remove(&from_label);
    sessions.insert(target_label.clone(), moved);
    drop(sessions);

    // Stash the source's serialized xterm buffer so the target
    // window's renderer can pull it on mount and write it into its
    // fresh terminal — preserves visible scrollback across the
    // tear-off. Empty strings are skipped (e.g. migration of a
    // fresh session before any data arrived).
    if let Some(snapshot) = terminal_snapshot {
        state.store_pending_terminal_snapshot(&target_label, snapshot);
    }

    // Fire a `serial:reconnected` to the target window so its
    // frontend updates whether its onMount has already run (in which
    // case its activeProfileID() pull returned empty and the
    // listener catches up here) or hasn't yet (in which case its
    // mount-time pull will see the migrated session and the event is
    // redundant). Same payload shape the existing reconnect handler
    // expects — the profile id of the live session.
    let app = window.app_handle().clone();
    let profile_id = state.with_session(&target_label, |handle| handle.profile_id.clone());
    if !profile_id.is_empty() {
        let _ = app.emit_to(target_label.as_str(), events::SERIAL_RECONNECTED, profile_id);
    }
    Ok(())
}

/// Whether the OS cursor is currently outside the calling window's
/// outer bounds. Phase-2 drag-to-spawn calls this on `dragend` — a
/// drop landed outside the source window is treated as a tear-off
/// and triggers [`open_profile_window`].
///
/// Both the cursor position and the window's outer rect come from
/// the OS in **physical pixels**, so DPI-scaling is consistent on
/// both sides of the comparison without the renderer doing
/// devicePixelRatio math.
#[tauri::command]
pub fn cursor_outside_window(window: WebviewWindow) -> Result<bool, String> {
    let cursor = window
        .app_handle()
        .cursor_position()
        .map_err(|e| format!("cursor_position: {}", e))?;
    let origin = window
        .outer_position()
        .map_err(|e| format!("outer_position: {}", e))?;
    let size = window
        .outer_size()
        .map_err(|e| format!("outer_size: {}", e))?;

    let left = origin.x as f64;
    let top = origin.y as f64;
    let right = left + size.width as f64;
    let bottom = top + size.height as f64;
    let inside = cursor.x >= left && cursor.x <= right && cursor.y >= top && cursor.y <= bottom;
    Ok(!inside)
}

/// Drain and return any pending xterm.js buffer snapshot for the
/// calling window. Migration sets one of these on the target's slot
/// just before the new window's renderer mounts; the renderer pulls
/// it once and writes it into the fresh terminal so the visible
/// scrollback survives the tear-off. Returns `None` for windows
/// that weren't created by a migration tear-off.
#[tauri::command]
pub fn take_pending_terminal_snapshot(
    window: WebviewWindow,
    state: State<'_, Arc<AppState>>,
) -> Option<String> {
    state.take_pending_terminal_snapshot(window.label())
}

/// Drain and return any pending initial profile id for the calling
/// window. `open_profile_window` stashes one of these for every
/// spawned window so the renderer can pre-select that profile on
/// mount. Returns `None` for the main window or for any spawned
/// window whose mount has already drained the value.
#[tauri::command]
pub fn take_pending_profile_id(
    window: WebviewWindow,
    state: State<'_, Arc<AppState>>,
) -> Option<String> {
    state.take_pending_profile_id(window.label())
}

/// Settings window label. Used by both the toggle command and the
/// destroy-event handler in lib.rs that persists final geometry.
pub const SETTINGS_WINDOW_LABEL: &str = "settings";

/// Default size when the Settings window opens for the first time
/// (no saved geometry in `settings.json`). Centered on screen via
/// the builder's default placement.
const SETTINGS_DEFAULT_WIDTH: f64 = 720.0;
const SETTINGS_DEFAULT_HEIGHT: f64 = 720.0;
const SETTINGS_MIN_WIDTH: f64 = 600.0;
const SETTINGS_MIN_HEIGHT: f64 = 500.0;

/// Open or close the dedicated Settings window. Singleton — if a
/// window labeled `settings` already exists, this command closes
/// it (per the shape brief's "toggle open/closed" choice). If no
/// such window exists, this opens one at the size/position remembered
/// from the previous session (or centered 720x720 the very first
/// time).
///
/// Returns the new state (`true` = window is now open, `false` =
/// window is now closed) so the caller can update local UI hints
/// (sidebar Settings button active state) immediately without
/// waiting for a window event round-trip.
#[tauri::command]
pub fn toggle_settings_window(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<bool, String> {
    // Perf instrumentation (alpha track only — strip when devtools
    // feature is dropped). Tagged `[perf]` so it greps cleanly out of
    // the rest of the log file. tauri-plugin-log forwards everything
    // to the JS DevTools console so all perf phases (Rust + JS) land
    // in the same stream.
    let ipc_enter = std::time::Instant::now();
    log::info!("[perf] settings-window: ipc-handler-enter");
    if let Some(existing) = app.get_webview_window(SETTINGS_WINDOW_LABEL) {
        // Singleton hit — toggle behavior closes the existing
        // window. The Destroyed event handler in lib.rs persists
        // the final geometry to settings.json on the way out.
        existing
            .close()
            .map_err(|e| format!("close settings window: {}", e))?;
        return Ok(false);
    }

    // Restore last known geometry from settings, or fall through to
    // a centered default. Width / height of zero (the field's
    // skip-serializing-default) means "not yet saved" — center the
    // window at default size in that case.
    let snapshot = state.settings.get();
    let saved = snapshot.settings_window.clone();
    let bg_color = background_color_for_appearance(&snapshot.appearance);
    let (saved_w, saved_h, saved_x, saved_y) = match saved {
        Some(g) => (g.width, g.height, g.x, g.y),
        None => (0, 0, 0, 0),
    };
    let width = if saved_w > 0 {
        saved_w as f64
    } else {
        SETTINGS_DEFAULT_WIDTH
    };
    let height = if saved_h > 0 {
        saved_h as f64
    } else {
        SETTINGS_DEFAULT_HEIGHT
    };

    // Build the window OFF the IPC dispatcher thread, matching the
    // pattern in open_profile_window. The synchronous IPC-handler
    // build pattern caused two problems on the previous commit:
    // (1) Windows deadlock (cf. v0.9.4-alpha.3 release notes), and
    // (2) on macOS, decorum's create_overlay_titlebar called from
    // inside the still-running IPC handler appears to fail to inject
    // its drag region cleanly — the result is a Settings window
    // that opens but won't drag. Spawning the build + decorum setup
    // to a tokio task lets the IPC handler return immediately, the
    // webview initializes on its own thread, and decorum's setup
    // runs against a fully-ready window.
    let app_clone = app.clone();
    tauri::async_runtime::spawn(async move {
        log::info!(
            "[perf] settings-window: task-start (after-spawn={}ms)",
            ipc_enter.elapsed().as_millis()
        );
        let build_start = std::time::Instant::now();
        let url = WebviewUrl::default();
        // Show-on-ready (see open_profile_window for full reasoning).
        // The Settings window benefits even more than profile windows
        // because it has more layered chrome (decorum titlebar,
        // .settings-window-shell wrapper) where the flash was most
        // visible.
        #[allow(unused_mut)]
        let mut builder = WebviewWindowBuilder::new(&app_clone, SETTINGS_WINDOW_LABEL, url)
            .title("Baudrun Settings")
            .inner_size(width, height)
            .min_inner_size(SETTINGS_MIN_WIDTH, SETTINGS_MIN_HEIGHT)
            .resizable(true)
            .visible(false)
            .background_color(bg_color);
        // macOS chrome treatment goes BEFORE positioning so the OS
        // window is created with the overlay-titlebar style baked in
        // from the start. Same order as open_profile_window.
        #[cfg(target_os = "macos")]
        {
            builder = builder
                .title_bar_style(tauri::TitleBarStyle::Overlay)
                .hidden_title(true);
        }
        if saved_x != 0 || saved_y != 0 {
            // Saved a non-(0,0) position — use it. (0, 0) is the
            // platform default for "no saved position", not a literal
            // top-left placement, so we treat it as missing.
            builder = builder.position(saved_x as f64, saved_y as f64);
        } else {
            builder = builder.center();
        }
        match builder.build() {
            Ok(_window) => {
                log::info!(
                    "[perf] settings-window: built (build={}ms total-since-ipc={}ms)",
                    build_start.elapsed().as_millis(),
                    ipc_enter.elapsed().as_millis()
                );
                // macOS-only chrome touch-ups via decorum, matching
                // the open_profile_window pattern. The underscore on
                // `_window` keeps the binding non-fatal on Linux /
                // Windows where the variable is unused (clippy runs
                // `-D warnings` in CI).
                #[cfg(target_os = "macos")]
                {
                    use tauri_plugin_decorum::WebviewWindowExt;
                    if let Err(err) = _window.create_overlay_titlebar() {
                        log::warn!(
                            "settings window: create_overlay_titlebar: {}",
                            err
                        );
                    }
                    if let Err(err) = _window.set_traffic_lights_inset(14.0, 20.0) {
                        log::warn!(
                            "settings window: set_traffic_lights_inset: {}",
                            err
                        );
                    }
                    log::info!("[perf] settings-window: chrome-applied");
                }
            }
            Err(err) => {
                log::error!("create settings window: {}", err);
            }
        }
    });
    Ok(true)
}

/// Persist the Settings window's current size + position to
/// `settings.json`. Called from the `WindowEvent::Destroyed` handler
/// in lib.rs when the user closes the Settings window — saving on
/// destroy, not on every Resized / Moved event, keeps the IO cost
/// flat regardless of how aggressively the user drags. Errors are
/// logged but don't propagate — failing to save geometry shouldn't
/// crash the close path.
pub fn persist_settings_window_geometry(
    app: &AppHandle,
    width: i32,
    height: i32,
    x: i32,
    y: i32,
) {
    let Some(state) = app.try_state::<Arc<AppState>>() else { return };
    let mut current = state.settings.get();
    current.settings_window = Some(crate::settings::WindowGeometry {
        width,
        height,
        x,
        y,
    });
    if let Err(err) = state.settings.update(current) {
        log::warn!("persist settings-window geometry: {}", err);
    }
}
