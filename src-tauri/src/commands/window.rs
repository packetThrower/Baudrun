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

    // `WebviewUrl::default()` resolves to an empty PathBuf which
    // Tauri serves as the dist root — same path the main window
    // uses (it has no explicit `url` in tauri.conf.json). Earlier
    // we passed `WebviewUrl::App("index.html".into())` explicitly,
    // but Windows release builds left the spawned window blank
    // even after the `?profile=` query was removed; matching the
    // main window's url-resolution path is the most reliable way
    // to avoid Windows-specific Tauri 2 quirks here.
    let url = WebviewUrl::default();
    // `title_bar_style` and `hidden_title` only exist on the macOS
    // builder API — chaining them unconditionally breaks the
    // Windows + Linux compiles (E0599: no method named ...). They
    // match the main window's overlay-titlebar treatment from
    // tauri.conf.json so spawned windows on macOS look identical to
    // main; non-macOS gets the default decorated chrome the conf
    // file describes for those platforms.
    //
    // `.devtools(true)` exposes WebView2 / WebKit / WebKitGTK
    // DevTools on the spawned window even in release builds — gated
    // behind the `tauri/devtools` feature in Cargo.toml. Useful for
    // diagnosing tear-off rendering issues that don't reproduce on
    // the maintainer's macOS dev box. Cheap to leave on for the
    // multi-window flow specifically; main window's devtools state
    // is unaffected.
    #[allow(unused_mut)]
    let mut builder = WebviewWindowBuilder::new(&app, &label, url)
        .title(title)
        .inner_size(1100.0, 720.0)
        .min_inner_size(800.0, 500.0)
        .devtools(true);
    #[cfg(target_os = "macos")]
    {
        builder = builder
            .title_bar_style(tauri::TitleBarStyle::Overlay)
            .hidden_title(true);
    }
    let window = builder
        .build()
        .map_err(|e| format!("create window: {}", e))?;

    // Match the main window's overlay-titlebar + traffic-light setup
    // so the spawned window doesn't look out of place. Failures here
    // aren't fatal — the window still opens, the chrome just looks
    // like the default.
    //
    // macOS-only — see the matching note in lib.rs's setup hook.
    // Calling decorum on Windows strips the native frame without
    // providing a CSS replacement, which hides the caption buttons
    // (issue #7).
    #[cfg(target_os = "macos")]
    {
        use tauri_plugin_decorum::WebviewWindowExt;
        if let Err(err) = window.create_overlay_titlebar() {
            log::warn!("spawned window {}: create_overlay_titlebar: {}", label, err);
        }
        if let Err(err) = window.set_traffic_lights_inset(14.0, 20.0) {
            log::warn!(
                "spawned window {}: set_traffic_lights_inset: {}",
                label,
                err
            );
        }
    }
    let _ = window.set_focus();

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
