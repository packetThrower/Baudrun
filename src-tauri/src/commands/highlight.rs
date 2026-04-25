//! Highlight-rule pack commands. Lets the frontend enumerate
//! every pack the user can enable (built-ins + the editable user
//! pack at `$SUPPORT_DIR/highlight-rules.json` + any imported packs
//! under `$SUPPORT_DIR/highlight/`), write changes back to the
//! scratchpad, and import / delete shared packs.

use std::sync::Arc;

use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

use crate::highlight::HighlightPack;
use crate::state::AppState;

#[tauri::command]
pub fn list_highlight_packs(state: State<'_, Arc<AppState>>) -> Vec<HighlightPack> {
    state.highlight.list()
}

#[tauri::command]
pub fn update_user_highlight_pack(
    pack: HighlightPack,
    state: State<'_, Arc<AppState>>,
) -> Result<HighlightPack, String> {
    state
        .highlight
        .update_user_pack(pack)
        .map_err(|e| e.to_string())
}

/// Pop a file picker, read the selected `.json`, validate it as a
/// highlight pack, and copy it into `$SUPPORT_DIR/highlight/<id>.json`.
/// The frontend should `listHighlightPacks()` again after a successful
/// import to refresh the Settings UI.
///
/// This is an **async** command: `blocking_pick_file` must not run on
/// the main thread, and Tauri 2 dispatches sync commands onto the main
/// thread. Making the command `async` drops it onto the async runtime,
/// so the blocking call waits on a worker while the main thread stays
/// free to render the native dialog — otherwise the UI beach-balls.
#[tauri::command]
pub async fn import_user_highlight_pack(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<HighlightPack, String> {
    // Clone the Arc out of State so we don't hold the (non-Send)
    // State reference across an await point.
    let state = state.inner().clone();

    let path = app
        .dialog()
        .file()
        .set_title("Import highlight pack")
        .add_filter("Highlight pack (JSON)", &["json"])
        .blocking_pick_file()
        .ok_or_else(|| "cancelled".to_string())?;

    let path_buf = path
        .into_path()
        .map_err(|e| format!("resolve path: {}", e))?;

    state
        .highlight
        .import_user_pack(&path_buf)
        .map_err(|e| e.to_string())
}

/// Remove an imported user pack. Scratchpad (`"user"`) and bundled
/// ids are rejected upstream in `Store::delete_user_pack`.
#[tauri::command]
pub fn delete_user_highlight_pack(
    id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    state
        .highlight
        .delete_user_pack(&id)
        .map_err(|e| e.to_string())
}
