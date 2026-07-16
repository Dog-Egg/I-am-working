#[derive(Clone, Debug, serde::Serialize, specta::Type)]
pub struct Stats {
    pub today_work_seconds: u64,
    pub is_active: bool,
    pub idle_seconds: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, specta::Type)]
pub struct HourlyWorkRecord {
    pub hour_start_unix: i64,
    pub work_seconds: u64,
}

#[derive(Clone, Debug, serde::Serialize, specta::Type, tauri_specta::Event)]
#[serde(transparent)]
pub struct StatsUpdated(pub Stats);
