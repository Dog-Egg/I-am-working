use std::collections::HashMap;
use std::time::Instant;

/// Runtime state owned by the work-timer feature.
pub(crate) struct WorkTimerState {
    pub(crate) is_active: bool,
    pub(crate) idle_started_at: Option<Instant>,
    pub(crate) pending_work_seconds_by_hour: HashMap<i64, u64>,
    pub(crate) last_flush_at: Instant,
    pub(crate) today_start_unix: i64,
    pub(crate) today_end_unix: i64,
    pub(crate) today_work_seconds: u64,
}

impl WorkTimerState {
    pub(crate) fn new(today_start_unix: i64, today_end_unix: i64, today_work_seconds: u64) -> Self {
        Self {
            is_active: false,
            idle_started_at: None,
            pending_work_seconds_by_hour: HashMap::new(),
            last_flush_at: Instant::now(),
            today_start_unix,
            today_end_unix,
            today_work_seconds,
        }
    }
}
