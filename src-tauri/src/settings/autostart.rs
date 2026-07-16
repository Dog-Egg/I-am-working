use tauri::AppHandle;
use tauri_plugin_autostart::ManagerExt;

use super::models::AppSettings;

pub(crate) fn sync_launch_at_login(app: &AppHandle, enabled: bool) -> Result<(), String> {
    let autolaunch = app.autolaunch();
    if enabled {
        autolaunch.enable()
    } else {
        autolaunch.disable()
    }
    .map_err(|error| error.to_string())
}

pub(crate) fn refresh_launch_at_login(app: &AppHandle, settings: &mut AppSettings) {
    if let Ok(enabled) = app.autolaunch().is_enabled() {
        settings.launch_at_login = enabled;
    }
}
