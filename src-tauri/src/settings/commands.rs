use std::sync::{Arc, Mutex};

use tauri::{AppHandle, State};

use crate::app::state::AppState;
use crate::features::nosleep::inhibitor::stop_sleep_inhibitor;
use crate::features::nosleep::tray::{create_nosleep_tray, remove_nosleep_tray};
use crate::features::work_timer::tray::update_tray_title;

use super::autostart::{refresh_launch_at_login, sync_launch_at_login};
use super::models::AppSettings;
use super::storage::persist_settings;

#[specta::specta]
#[tauri::command]
pub(crate) fn get_settings(app: AppHandle, state: State<'_, Arc<Mutex<AppState>>>) -> AppSettings {
    let state = state.lock().unwrap();
    let mut settings = state.settings.settings.clone();
    refresh_launch_at_login(&app, &mut settings);
    settings
}

#[specta::specta]
#[tauri::command]
pub(crate) fn update_settings(
    app: AppHandle,
    state: State<'_, Arc<Mutex<AppState>>>,
    settings: AppSettings,
) -> Result<AppSettings, String> {
    let should_enable_nosleep = settings.nosleep_enabled;
    let was_nosleep_enabled = state.lock().unwrap().settings.settings.nosleep_enabled;
    if should_enable_nosleep && !was_nosleep_enabled {
        if let Err(error) = create_nosleep_tray(&app) {
            return Err(error.to_string());
        }
    }

    let settings_result = (|| {
        let mut state = state.lock().unwrap();
        sync_launch_at_login(&app, settings.launch_at_login)?;
        persist_settings(&state.settings.settings_path, &settings)
            .map_err(|error| error.to_string())?;
        state.settings.settings = settings;
        Ok::<_, String>(state.settings.settings.clone())
    })();

    let next_settings = match settings_result {
        Ok(result) => result,
        Err(error) => {
            if should_enable_nosleep && !was_nosleep_enabled {
                remove_nosleep_tray(&app);
            }
            return Err(error);
        }
    };

    let today_work_seconds = state.lock().unwrap().work_timer.today_work_seconds;
    update_tray_title(&app, today_work_seconds, &next_settings);
    if was_nosleep_enabled && !next_settings.nosleep_enabled {
        if let Ok(mut state) = state.lock() {
            state.nosleep.active_guards.clear();
        }
        stop_sleep_inhibitor();
        remove_nosleep_tray(&app);
    }

    Ok(next_settings)
}
