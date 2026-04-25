//! Window chrome + multi-window commands.
//!
//! - `set_traffic_lights_inset`: macOS-specific traffic-light
//!   repositioning via `tauri-plugin-decorum` so skins with a
//!   floating-bubble layout (macos-26 Liquid Glass) can pull the
//!   lights inside the panel. Non-macOS platforms accept the call
//!   and no-op.
//! - `open_profile_window`: spawn a new top-level webview pointing
//!   at the same renderer URL with `?profile=<id>` so the new
//!   window's frontend lands on that profile selected. The new
//!   window has its own session in [`crate::state::AppState`] keyed
//!   by its label, so connecting / disconnecting / transferring on
//!   one window doesn't disturb the others.

use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};
use uuid::Uuid;

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
) -> Result<String, String> {
    // Sanitize the profile id into a URL-safe slice — Tauri's URL is
    // assembled from the renderer's index.html plus our query string
    // and the renderer parses it back via URLSearchParams on mount.
    // Reject anything fishy up front so the value can't break out of
    // the query-string scope on either end.
    let safe_id: String = profile_id
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .collect();
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

    let url = WebviewUrl::App(format!("index.html?profile={}", safe_id).into());
    // `title_bar_style` and `hidden_title` only exist on the macOS
    // builder API — chaining them unconditionally breaks the
    // Windows + Linux compiles (E0599: no method named ...). They
    // match the main window's overlay-titlebar treatment from
    // tauri.conf.json so spawned windows on macOS look identical to
    // main; non-macOS gets the default decorated chrome the conf
    // file describes for those platforms.
    #[allow(unused_mut)]
    let mut builder = WebviewWindowBuilder::new(&app, &label, url)
        .title(title)
        .inner_size(1100.0, 720.0)
        .min_inner_size(800.0, 500.0);
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
    #[cfg(desktop)]
    {
        use tauri_plugin_decorum::WebviewWindowExt;
        if let Err(err) = window.create_overlay_titlebar() {
            log::warn!("spawned window {}: create_overlay_titlebar: {}", label, err);
        }
        #[cfg(target_os = "macos")]
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
