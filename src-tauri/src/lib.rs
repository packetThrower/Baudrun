use std::sync::{Arc, Mutex};

use tauri::Manager;

pub mod appdata;
pub mod commands;
pub mod events;
pub mod profiles;
pub mod serial;
pub mod settings;
pub mod skins;
pub mod state;
pub mod themes;
pub mod transfer;

use state::{AppState, SessionHandle};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default();

    #[cfg(desktop)]
    {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }));
    }

    builder
        .plugin(tauri_plugin_log::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // Initialize stores and install shared state. A failure
            // here is fatal — the app can't run without at least the
            // profiles / settings stores, and the frontend can't
            // usefully render a "backend init failed" dialog without
            // them either, so we return an error and let Tauri abort.
            let support_dir = appdata::support_dir()
                .map_err(|e| format!("resolve support dir: {}", e))?;

            let profiles = profiles::Store::new(&support_dir)
                .map_err(|e| format!("profile store init: {}", e))?;
            let settings = settings::Store::new(&support_dir)
                .map_err(|e| format!("settings store init: {}", e))?;
            let themes = themes::Store::new(&support_dir)
                .map_err(|e| format!("theme store init: {}", e))?;
            let skins = skins::Store::new(&support_dir)
                .map_err(|e| format!("skin store init: {}", e))?;

            let app_state = Arc::new(AppState {
                profiles,
                settings,
                themes,
                skins,
                session: Mutex::new(SessionHandle::default()),
            });
            app.manage(app_state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // profiles
            commands::profiles::list_profiles,
            commands::profiles::create_profile,
            commands::profiles::update_profile,
            commands::profiles::delete_profile,
            commands::profiles::default_profile,
            // themes
            commands::themes::list_themes,
            commands::themes::import_theme,
            commands::themes::delete_theme,
            // skins
            commands::skins::list_skins,
            commands::skins::import_skin,
            commands::skins::delete_skin,
            // settings + paths
            commands::settings::get_settings,
            commands::settings::update_settings,
            commands::settings::pick_log_directory,
            commands::settings::default_log_directory,
            commands::settings::get_config_directory,
            commands::settings::get_default_config_directory,
            commands::settings::pick_config_directory,
            commands::settings::set_config_directory,
            commands::settings::open_path,
            // serial
            commands::serial::list_ports,
            commands::serial::list_missing_drivers,
            commands::serial::connect,
            commands::serial::disconnect,
            commands::serial::send,
            commands::serial::set_rts,
            commands::serial::set_dtr,
            commands::serial::send_break,
            commands::serial::active_profile_id,
            commands::serial::get_control_lines,
            // transfer
            commands::transfer::pick_send_file,
            commands::transfer::send_file,
            commands::transfer::cancel_transfer,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
