use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::Instant;

use rusqlite::Connection;

use crate::settings::models::AppSettings;

/// Shared runtime container managed by Tauri.
///
/// Feature-specific behavior lives in the corresponding feature modules; this
/// type only owns the data that must be shared through Tauri state.
pub(crate) struct AppState {
    pub(crate) is_active: bool,
    pub(crate) active_guards: HashSet<String>,
    pub(crate) idle_started_at: Option<Instant>,
    pub(crate) pending_work_seconds_by_hour: HashMap<i64, u64>,
    pub(crate) last_flush_at: Instant,
    pub(crate) today_start_unix: i64,
    pub(crate) today_end_unix: i64,
    pub(crate) today_work_seconds: u64,
    pub(crate) settings: AppSettings,
    pub(crate) settings_path: PathBuf,
    pub(crate) db: Connection,
}
