use std::sync::{Arc, Mutex};

use tauri::State;

use crate::app::state::AppState;

use super::models::{HourlyWorkRecord, Stats};
use super::storage::work_records_in_range;
use super::ticker::build_stats;

#[specta::specta]
#[tauri::command]
pub(crate) fn get_stats(state: State<'_, Arc<Mutex<AppState>>>) -> Stats {
    let state = state.lock().unwrap();
    build_stats(&state.work_timer)
}

#[specta::specta]
#[tauri::command]
pub(crate) fn get_work_records(
    state: State<'_, Arc<Mutex<AppState>>>,
    start_unix: i64,
    end_unix: i64,
) -> Result<Vec<HourlyWorkRecord>, String> {
    let state = state.lock().unwrap();
    work_records_in_range(&state.work_timer, &state.db, start_unix, end_unix)
        .map_err(|error| error.to_string())
}
