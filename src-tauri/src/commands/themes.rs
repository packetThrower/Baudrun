//! Theme commands. Import uses `tauri-plugin-dialog` to present the
//! native file picker for an `.itermcolors` XML plist, then hands
//! the selected path to `themes::Store::import`.

use std::sync::Arc;

use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

use crate::state::AppState;
use crate::themes::Theme;

#[tauri::command]
pub fn list_themes(state: State<'_, Arc<AppState>>) -> Vec<Theme> {
    state.themes.list()
}

/// `async` is load-bearing — see `pick_send_file` in transfer.rs for
/// the long version. Short version: a sync command's `blocking_pick_file`
/// freezes the UI on Linux / Windows because Tauri 2 runs sync commands
/// on the WebView main thread.
#[tauri::command]
pub async fn import_theme(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<Theme, String> {
    // Clone the Arc out of State so we don't hold the (non-Send)
    // State reference across an await point.
    let state = state.inner().clone();

    let path = app
        .dialog()
        .file()
        .set_title("Import iTerm2 color scheme")
        .add_filter("iTerm2 Color Schemes", &["itermcolors"])
        .blocking_pick_file()
        .ok_or_else(|| "cancelled".to_string())?;

    let path_buf = path
        .into_path()
        .map_err(|e| format!("resolve path: {}", e))?;

    state
        .themes
        .import(&path_buf)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_theme(id: String, state: State<'_, Arc<AppState>>) -> Result<(), String> {
    state.themes.delete(&id).map_err(|e| e.to_string())
}
