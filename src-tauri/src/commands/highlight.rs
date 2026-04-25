//! Highlight-rule pack commands. Lets the frontend enumerate
//! every pack the user can enable (built-ins + the editable user
//! pack at `$SUPPORT_DIR/highlight-rules.json`) and write changes
//! back to the user pack.

use std::sync::Arc;

use tauri::State;

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
