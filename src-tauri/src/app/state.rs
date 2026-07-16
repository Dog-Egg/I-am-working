use rusqlite::Connection;

use crate::features::nosleep::state::NoSleepState;
use crate::features::work_timer::state::WorkTimerState;
use crate::settings::state::SettingsState;

/// Application-level state container.
///
/// Feature runtime data stays grouped by feature, while shared infrastructure
/// such as the database connection is owned at the application level.
pub(crate) struct AppState {
    pub(crate) work_timer: WorkTimerState,
    pub(crate) nosleep: NoSleepState,
    pub(crate) settings: SettingsState,
    pub(crate) db: Connection,
}
