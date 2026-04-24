//! Skin commands. Import picks a Baudrun skin JSON file via the
//! native dialog and hands it to `skins::Store::import`.

use std::sync::Arc;

use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

use crate::skins::Skin;
use crate::state::AppState;

#[tauri::command]
pub fn list_skins(state: State<'_, Arc<AppState>>) -> Vec<Skin> {
    state.skins.list()
}

#[tauri::command]
pub fn import_skin(app: AppHandle, state: State<'_, Arc<AppState>>) -> Result<Skin, String> {
    let path = app
        .dialog()
        .file()
        .set_title("Import skin")
        .add_filter("Baudrun skin", &["json"])
        .blocking_pick_file()
        .ok_or_else(|| "cancelled".to_string())?;

    let path_buf = path
        .into_path()
        .map_err(|e| format!("resolve path: {}", e))?;

    state
        .skins
        .import(&path_buf)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_skin(id: String, state: State<'_, Arc<AppState>>) -> Result<(), String> {
    state.skins.delete(&id).map_err(|e| e.to_string())
}
