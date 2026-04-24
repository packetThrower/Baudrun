//! Tauri command handlers grouped by domain. Individual submodules
//! hold the `#[tauri::command]` functions; this module just re-
//! exports them for the invoke_handler list in `lib.rs`.

pub mod profiles;
pub mod serial;
pub mod settings;
pub mod skins;
pub mod themes;
pub mod transfer;
