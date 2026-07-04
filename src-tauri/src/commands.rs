use std::sync::{Arc, Mutex};

use tauri::{AppHandle, State};

use crate::app_state::{build_stats, AppState};
use crate::desktop::{refresh_launch_at_login, sync_launch_at_login, update_tray_title};
use crate::models::{AppSettings, HourlyWorkRecord, Stats};
use crate::storage::{persist_settings, work_records_in_range};

#[specta::specta]
#[tauri::command]
pub(crate) fn get_stats(state: State<'_, Arc<Mutex<AppState>>>) -> Stats {
    let s = state.lock().unwrap();
    build_stats(&s)
}

#[specta::specta]
#[tauri::command]
pub(crate) fn get_work_records(
    state: State<'_, Arc<Mutex<AppState>>>,
    start_unix: i64,
    end_unix: i64,
) -> Result<Vec<HourlyWorkRecord>, String> {
    let s = state.lock().unwrap();
    work_records_in_range(&s, start_unix, end_unix).map_err(|e| e.to_string())
}

#[specta::specta]
#[tauri::command]
pub(crate) fn get_settings(app: AppHandle, state: State<'_, Arc<Mutex<AppState>>>) -> AppSettings {
    let s = state.lock().unwrap();
    let mut settings = s.settings.clone();
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
    let (today_work_seconds, next_settings) = {
        let mut s = state.lock().unwrap();
        sync_launch_at_login(&app, settings.launch_at_login)?;
        persist_settings(&s.settings_path, &settings).map_err(|e| e.to_string())?;
        s.settings = settings;
        (s.today_work_seconds, s.settings.clone())
    };

    update_tray_title(&app, today_work_seconds, &next_settings);

    Ok(next_settings)
}
