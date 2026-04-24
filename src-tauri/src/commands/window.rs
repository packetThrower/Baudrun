//! Window chrome commands. macOS-specific traffic-light
//! repositioning via `tauri-plugin-decorum` so skins with a
//! floating-bubble layout (macos-26 Liquid Glass) can pull the
//! lights inside the panel instead of leaving them stranded in the
//! shell-padding background. Non-macOS platforms accept the call
//! and no-op.

use tauri::WebviewWindow;

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
