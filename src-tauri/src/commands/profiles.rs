//! Profile CRUD commands. Each command is a thin wrapper around the
//! `profiles::Store` methods, converting typed errors into a
//! `Result<_, String>` for IPC.

use std::sync::Arc;

use tauri::State;

use crate::profiles::Profile;
use crate::state::AppState;

#[tauri::command]
pub fn list_profiles(state: State<'_, Arc<AppState>>) -> Vec<Profile> {
    state.profiles.list()
}

#[tauri::command]
pub fn create_profile(profile: Profile, state: State<'_, Arc<AppState>>) -> Result<Profile, String> {
    state.profiles.create(profile).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_profile(profile: Profile, state: State<'_, Arc<AppState>>) -> Result<Profile, String> {
    state.profiles.update(profile).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_profile(id: String, state: State<'_, Arc<AppState>>) -> Result<(), String> {
    state.profiles.delete(&id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn default_profile() -> Profile {
    Profile::defaults()
}
