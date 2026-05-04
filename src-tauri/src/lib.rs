use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tauri::{Manager, RunEvent, WindowEvent};

pub mod appdata;
pub mod commands;
pub mod events;
pub mod highlight;
pub mod profiles;
pub mod sanitize;
pub mod serial;
pub mod settings;
pub mod skins;
pub mod state;
pub mod themes;
pub mod transfer;
pub mod usbserial;

use state::AppState;

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
        .plugin(tauri_plugin_decorum::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
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
            let highlight = highlight::Store::new(&support_dir)
                .map_err(|e| format!("highlight store init: {}", e))?;

            let app_state = Arc::new(AppState {
                profiles,
                settings,
                themes,
                skins,
                highlight,
                sessions: Mutex::new(HashMap::new()),
                pending_terminal_snapshots: Mutex::new(HashMap::new()),
                pending_profile_ids: Mutex::new(HashMap::new()),
            });
            app.manage(app_state);

            // Wire decorum's overlay titlebar (injects an invisible
            // drag strip at DOM page-load and subclasses NSWindow so
            // traffic-light repositioning sticks across resizes).
            // Initial inset matches the default Baudrun skin; skin
            // changes adjust it via the set_traffic_lights_inset
            // command.
            //
            // macOS-only: on Windows decorum strips the native frame
            // and expects the renderer to draw its own titlebar — we
            // don't, so calling it there hides the system caption
            // buttons (issue #7). Linux WebKit2GTK is similar. Both
            // platforms get the default decorated chrome instead.
            #[cfg(target_os = "macos")]
            if let Some(win) = app.get_webview_window("main") {
                use tauri_plugin_decorum::WebviewWindowExt;
                win.create_overlay_titlebar()
                    .map_err(|e| format!("create_overlay_titlebar: {}", e))?;
                win.set_traffic_lights_inset(14.0, 20.0)
                    .map_err(|e| format!("set_traffic_lights_inset: {}", e))?;
            }
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
            // window chrome + multi-window
            commands::window::set_traffic_lights_inset,
            commands::window::open_profile_window,
            commands::window::cursor_outside_window,
            commands::window::migrate_session,
            commands::window::take_pending_terminal_snapshot,
            commands::window::take_pending_profile_id,
            commands::window::toggle_settings_window,
            // highlight rules
            commands::highlight::list_highlight_packs,
            commands::highlight::update_user_highlight_pack,
            commands::highlight::import_user_highlight_pack,
            commands::highlight::delete_user_highlight_pack,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            let RunEvent::WindowEvent { label, event, .. } = &event else {
                return;
            };
            match event {
                // CloseRequested fires BEFORE the window is gone so we
                // can still query its size/position. Stash those for
                // the Settings window so it reopens at the same place
                // next launch. We don't `prevent_close()` — the close
                // proceeds normally; we just ride along.
                WindowEvent::CloseRequested { .. }
                    if label == commands::window::SETTINGS_WINDOW_LABEL =>
                {
                    if let Some(window) = app.get_webview_window(label) {
                        let scale = window.scale_factor().unwrap_or(1.0);
                        let size = window
                            .inner_size()
                            .ok()
                            .map(|s| s.to_logical::<f64>(scale));
                        let pos = window
                            .outer_position()
                            .ok()
                            .map(|p| p.to_logical::<f64>(scale));
                        if let (Some(s), Some(p)) = (size, pos) {
                            commands::window::persist_settings_window_geometry(
                                app,
                                s.width as i32,
                                s.height as i32,
                                p.x as i32,
                                p.y as i32,
                            );
                        }
                    }
                }
                // Drop the per-window session map entry on window close
                // so a torn-off window's serial port is freed even if
                // the user quits via the red close button rather than
                // Disconnect → Quit. Tauri delivers Destroyed AFTER the
                // OS-level close, so the session's underlying SerialPort
                // is already closing when this fires; here we just clear
                // the bookkeeping.
                WindowEvent::Destroyed => {
                    if let Some(state) = app.try_state::<Arc<AppState>>() {
                        state.forget_session(label);
                    }
                }
                _ => {}
            }
        });
}
