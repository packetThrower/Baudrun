//! Settings commands. Covers global settings CRUD plus the config-
//! directory relocation machinery and the "open in file manager"
//! helper. All path-related operations use Tauri plugins where
//! possible (`tauri-plugin-dialog`, `tauri-plugin-opener`).

use std::path::PathBuf;
use std::sync::Arc;

use tauri::{AppHandle, Emitter, State};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;

use crate::appdata;
use crate::events;
use crate::settings::Settings;
use crate::state::AppState;

#[tauri::command]
pub fn get_settings(state: State<'_, Arc<AppState>>) -> Settings {
    state.settings.get()
}

#[tauri::command]
pub fn update_settings(
    settings: Settings,
    state: State<'_, Arc<AppState>>,
    app: AppHandle,
) -> Result<Settings, String> {
    let updated = state
        .settings
        .update(settings)
        .map_err(|e| e.to_string())?;
    // Broadcast to every window so each renderer can refresh its
    // local settings store. Without this, the Settings window's
    // edits stay isolated to that window — e.g. changing the skin
    // would only repaint Settings, not main. Logged-and-swallowed
    // because a failed event-emit shouldn't fail the actual settings
    // write that already succeeded on disk.
    if let Err(err) = app.emit(events::SETTINGS_UPDATED, &updated) {
        log::warn!("emit {}: {}", events::SETTINGS_UPDATED, err);
    }
    Ok(updated)
}

/// `async` is load-bearing — see `pick_send_file` in transfer.rs for
/// the long version. Short version: a sync command's `blocking_pick_folder`
/// freezes the UI on Linux / Windows because Tauri 2 runs sync commands
/// on the WebView main thread.
#[tauri::command]
pub async fn pick_log_directory(app: AppHandle) -> Result<String, String> {
    pick_directory(&app, "Choose session log directory")
}

/// Default log directory — `$SUPPORT_DIR/logs`. Shown as a hint in
/// the Settings UI when no LogDir is configured.
#[tauri::command]
pub fn default_log_directory() -> Result<String, String> {
    let support = appdata::support_dir().map_err(|e| e.to_string())?;
    Ok(support.join("logs").to_string_lossy().into_owned())
}

#[tauri::command]
pub fn get_config_directory() -> Result<String, String> {
    appdata::support_dir()
        .map(|p| p.to_string_lossy().into_owned())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_default_config_directory() -> Result<String, String> {
    appdata::default_support_dir()
        .map(|p| p.to_string_lossy().into_owned())
        .map_err(|e| e.to_string())
}

/// `async` is load-bearing — see `pick_send_file` in transfer.rs for
/// the long version. Short version: a sync command's `blocking_pick_folder`
/// freezes the UI on Linux / Windows because Tauri 2 runs sync commands
/// on the WebView main thread.
#[tauri::command]
pub async fn pick_config_directory(app: AppHandle) -> Result<String, String> {
    pick_directory(&app, "Choose config directory")
}

/// Write or clear the config-directory override. Empty string clears
/// it — next launch uses the default. Current session keeps reading
/// from the path it was started with; relocation takes effect on
/// restart.
#[tauri::command]
pub fn set_config_directory(dir: String) -> Result<(), String> {
    let path = if dir.is_empty() {
        None
    } else {
        Some(PathBuf::from(&dir))
    };
    appdata::write_override(path.as_deref()).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn open_path(path: String, app: AppHandle) -> Result<(), String> {
    if path.is_empty() {
        return Err("empty path".into());
    }
    app.opener()
        .open_path(path, None::<&str>)
        .map_err(|e| e.to_string())
}

fn pick_directory(app: &AppHandle, title: &str) -> Result<String, String> {
    let path = app
        .dialog()
        .file()
        .set_title(title)
        .blocking_pick_folder();
    match path {
        Some(p) => p
            .into_path()
            .map(|pb| pb.to_string_lossy().into_owned())
            .map_err(|e| format!("resolve path: {}", e)),
        // Cancel returns empty string rather than an error — matches
        // the Go Wails behaviour the frontend already knows how to
        // interpret.
        None => Ok(String::new()),
    }
}
